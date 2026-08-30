//! Best-effort, read-only hardware/session detection.
//!
//! Detection only — the relevance *filter* that consumes this lives in
//! `src/lib/util.ts irrelevance()` and has no Rust counterpart. There used to be
//! one here; it went three capability tags stale and rotted into dead code,
//! because `hdr`/`fsr4`/`rdna3`/`rdna4` are opt-in settings held in the frontend
//! store that never reach this side. `lint.rs` is the one Rust consumer, and it
//! reads the fields directly.
//!
//! GPU architecture *is* now detected, best-effort, via the PCI id — but only
//! as [`Hardware::gpu_gen_detected`], a suggestion. `store.gpu_gen` remains the
//! user's declaration and always outranks it; see `effectiveGpuGen` in
//! `state.svelte.ts`, which is where the two are reconciled.

use std::path::Path;

use serde::Serialize;

use crate::which;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Hardware {
    pub nvidia: bool,
    pub amd: bool,
    pub intel: bool,
    pub wayland: bool,
    pub kde: bool,
    pub ntsync: bool,
    /// `PRETTY_NAME` from `/etc/os-release`, "" if unreadable. Context for the
    /// LLM prompt only — see [`Self::llm_context`]; never used for relevance
    /// filtering (that stays in the frontend, see the module doc comment).
    pub distro: String,
    /// `/proc/sys/kernel/osrelease`, trimmed. "" if unreadable.
    pub kernel: String,
    /// Total RAM in GiB, from `/proc/meminfo`'s `MemTotal`. 0 if unreadable.
    pub ram_gb: u32,
    /// First "model name" line of `/proc/cpuinfo`. "" if unreadable.
    pub cpu_model: String,
    /// Best-effort RDNA generation of the installed AMD GPU: `"rdna3"`,
    /// `"rdna4"`, or `None` for anything older, non-AMD, or unrecognised.
    ///
    /// A **suggestion, never an override.** `store.gpu_gen` is what the user
    /// declared and always wins; this only fills in when they have declared
    /// nothing, so the FSR/RDNA options stop being unreachable-by-default on a
    /// machine that plainly qualifies. Detection is genuinely best-effort — it
    /// needs hwdata's `pci.ids` on disk and a `Navi <n>` codename in the entry —
    /// which is exactly why it must not overrule an explicit choice.
    pub gpu_gen_detected: Option<String>,
}

/// AMD's PCI vendor id, as sysfs spells it.
const AMD_VENDOR: &str = "0x1002";

/// Where distros install hwdata's PCI id database.
///
/// Read from disk rather than embedding a device-id table: such a table needs
/// hand-maintenance every GPU generation and silently misreports new cards until
/// someone remembers, whereas this file already ships with the distro and is
/// already kept current by it.
const PCI_IDS_PATHS: [&str; 2] = ["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids"];

/// The RDNA generation a `pci.ids` device name implies.
///
/// Bucketed by the `Navi <n>` codename's leading digit — Navi 1x is RDNA1, 2x
/// RDNA2, 3x RDNA3, 4x RDNA4 — which is the one part of these names AMD has kept
/// systematic. The marketing suffix is not: the same entry covers
/// `RX 7900 XT/7900 XTX/7900 GRE/7900M`, and matching on it would be a guessing
/// game.
///
/// Only the two generations the catalog gates on are reported. Everything else
/// is `None`, which reads identically to "not declared" and leaves every FSR row
/// hidden — the safe direction.
fn generation_from_name(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    let at = lower.find("navi ")?;
    let digits: String = lower[at + "navi ".len()..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    // Two digits exactly: "Navi 31", never a bare "Navi 3" or a stray "Navi 100".
    if digits.len() != 2 {
        return None;
    }
    match digits.as_bytes()[0] {
        b'3' => Some("rdna3"),
        b'4' => Some("rdna4"),
        _ => None,
    }
}

/// Look up a device name in `pci.ids` text.
///
/// The format is column-significant: vendor lines start at column 0, their
/// devices are indented one tab, and subsystem lines two. `device` is lowercase
/// hex without the `0x` prefix.
fn pci_ids_lookup(text: &str, vendor: &str, device: &str) -> Option<String> {
    let mut in_vendor = false;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if !line.starts_with('\t') {
            // Reached the next vendor block: ours had no such device.
            if in_vendor {
                return None;
            }
            in_vendor = line.split_whitespace().next() == Some(vendor);
            continue;
        }
        // Two tabs is a subsystem line, which names a board vendor, not the chip.
        if !in_vendor || line.starts_with("\t\t") {
            continue;
        }
        let Some((id, name)) = line.trim_start().split_once(char::is_whitespace) else {
            continue;
        };
        if id.eq_ignore_ascii_case(device) {
            return Some(name.trim().to_string());
        }
    }
    None
}

/// PCI device ids of every AMD GPU with a DRM card node, lowercase hex without
/// the `0x` prefix, ordered by card number.
///
/// **All** of them, not just the first: a Ryzen desktop or laptop exposes its
/// integrated display as `card0` and the discrete card as `card1`, and `readdir`
/// order is arbitrary anyway. Returning one would have made this machine —
/// `card0` = `13c0` "Granite Ridge [Radeon Graphics]`, `card1` = `7590`
/// "Navi 44 [Radeon RX 9060 XT]" — detect nothing at all.
fn amd_pci_devices() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let file = entry.file_name();
        let Some(name) = file.to_str() else { continue };
        // `card0` is a GPU; `card0-DP-1` is one of its connectors.
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let dir = entry.path().join("device");
        let read = |f: &str| {
            std::fs::read_to_string(dir.join(f))
                .ok()
                .map(|s| s.trim().to_ascii_lowercase())
        };
        let (Some(vendor), Some(device)) = (read("vendor"), read("device")) else {
            continue;
        };
        if vendor == AMD_VENDOR {
            found.push((name.to_string(), device.trim_start_matches("0x").to_string()));
        }
    }
    // `readdir` order is not the card order; sort so the answer is stable.
    found.sort();
    found.into_iter().map(|(_, device)| device).collect()
}

/// Best-effort RDNA generation of the installed AMD GPU: the first card whose
/// PCI id names a generation we recognise.
///
/// "First *recognised*", not "first card", is what skips an integrated Radeon
/// (`Granite Ridge`, `Raphael` — no `Navi <n>` codename) in favour of the
/// discrete card sitting behind it. `None` whenever nothing in the chain
/// resolves: no AMD card, no `pci.ids` on disk, or no recognised codename.
fn detect_gpu_gen() -> Option<String> {
    let devices = amd_pci_devices();
    if devices.is_empty() {
        return None;
    }
    for path in PCI_IDS_PATHS {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let found = devices
            .iter()
            .filter_map(|d| pci_ids_lookup(&text, "1002", d))
            .find_map(|name| generation_from_name(&name));
        if let Some(generation) = found {
            return Some(generation.to_string());
        }
    }
    None
}

fn module_loaded(name: &str) -> bool {
    Path::new("/sys/module").join(name).is_dir()
}

/// Best-effort read of `/etc/os-release`'s `PRETTY_NAME=` value.
fn read_distro() -> String {
    let Ok(text) = std::fs::read_to_string("/etc/os-release") else { return String::new() };
    text.lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_default()
}

/// Best-effort read of the running kernel release string.
fn read_kernel() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Best-effort read of total RAM, in GiB, from `/proc/meminfo`.
fn read_ram_gb() -> u32 {
    let Ok(text) = std::fs::read_to_string("/proc/meminfo") else { return 0 };
    text.lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .and_then(|v| v.trim().split_whitespace().next())
        .and_then(|kib| kib.parse::<u64>().ok())
        .map(|kib| (kib / 1024 / 1024) as u32)
        .unwrap_or(0)
}

/// Best-effort read of the CPU model name from the first `/proc/cpuinfo` entry.
fn read_cpu_model() -> String {
    let Ok(text) = std::fs::read_to_string("/proc/cpuinfo") else { return String::new() };
    text.lines()
        .find_map(|l| l.strip_prefix("model name"))
        .and_then(|v| v.split_once(':'))
        .map(|(_, name)| name.trim().to_string())
        .unwrap_or_default()
}

pub fn detect() -> Hardware {
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    Hardware {
        nvidia: module_loaded("nvidia") || which::is_installed("nvidia-smi"),
        amd: module_loaded("amdgpu"),
        intel: module_loaded("i915") || module_loaded("xe"),
        wayland: session.eq_ignore_ascii_case("wayland")
            || std::env::var_os("WAYLAND_DISPLAY").is_some(),
        kde: desktop.to_uppercase().contains("KDE"),
        ntsync: Path::new("/dev/ntsync").exists(),
        distro: read_distro(),
        kernel: read_kernel(),
        ram_gb: read_ram_gb(),
        cpu_model: read_cpu_model(),
        gpu_gen_detected: detect_gpu_gen(),
    }
}

impl Hardware {
    /// One-line description for the `--list` CLI.
    pub fn summary(&self) -> String {
        let mut gpus = Vec::new();
        if self.nvidia {
            gpus.push("NVIDIA");
        }
        if self.amd {
            gpus.push("AMD");
        }
        if self.intel {
            gpus.push("Intel");
        }
        let mut gpu = if gpus.is_empty() { "unknown GPU".to_string() } else { gpus.join("+") };
        // The detected RDNA generation rides along here rather than getting its
        // own line: it qualifies the GPU, and this string is both the `--list`
        // summary and the first line of the LLM's hardware context, where
        // "AMD (RDNA4)" is materially better advice-shaping than "AMD".
        if let Some(generation) = &self.gpu_gen_detected {
            gpu = format!("{gpu} ({})", generation.to_uppercase());
        }
        let session = if self.wayland { "Wayland" } else { "X11" };
        let kde = if self.kde { ", KDE" } else { "" };
        let ntsync = if self.ntsync { ", ntsync" } else { "" };
        format!("{gpu}, {session}{kde}{ntsync}")
    }

    /// Fuller context for the LLM prompt: the one-line [`Self::summary`] plus
    /// distro, kernel and system specs the model can't infer from a log alone.
    /// Not used by the `--list` CLI — that stays on `summary()`.
    pub fn llm_context(&self) -> String {
        let mut lines = vec![self.summary()];
        if !self.distro.is_empty() {
            lines.push(format!("Distro: {}", self.distro));
        }
        if !self.kernel.is_empty() {
            lines.push(format!("Kernel: {}", self.kernel));
        }
        if !self.cpu_model.is_empty() {
            lines.push(format!("CPU: {}", self.cpu_model));
        }
        if self.ram_gb > 0 {
            lines.push(format!("RAM: {} GB", self.ram_gb));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A slice of the real `pci.ids` layout: vendor lines at column 0, devices
    /// one tab in, subsystems two.
    const PCI_IDS_FIXTURE: &str = "\
# Comment line
1002  Advanced Micro Devices, Inc. [AMD/ATI]
\t73df  Navi 22 [Radeon RX 6700/6700 XT/6750 XT / 6800M/6850M XT]
\t744c  Navi 31 [Radeon RX 7900 XT/7900 XTX/7900 GRE/7900M]
\t\t1002 0e3b  Radeon RX 7900 XTX
\t7550  Navi 48 [Radeon RX 9070/9070 XT]
\t164e  Raphael
10de  NVIDIA Corporation
\t2684  AD102 [GeForce RTX 4090]
";

    #[test]
    fn generation_comes_from_the_navi_codename_not_the_marketing_name() {
        assert_eq!(
            generation_from_name("Navi 31 [Radeon RX 7900 XT/7900 XTX/7900 GRE/7900M]"),
            Some("rdna3")
        );
        assert_eq!(generation_from_name("Navi 48 [Radeon RX 9070/9070 XT]"), Some("rdna4"));
        // Older generations are deliberately not reported: the catalog gates
        // only on rdna3/rdna4, and `None` reads the same as "not declared".
        assert_eq!(generation_from_name("Navi 22 [Radeon RX 6700/6700 XT]"), None);
        assert_eq!(generation_from_name("Navi 10 [Radeon RX 5600 XT]"), None);
    }

    #[test]
    fn an_unrecognised_name_is_none_rather_than_a_guess() {
        // No codename at all (integrated parts, and whatever AMD names next).
        assert_eq!(generation_from_name("Raphael"), None);
        assert_eq!(generation_from_name("AD102 [GeForce RTX 4090]"), None);
        assert_eq!(generation_from_name(""), None);
        // A bare single digit must not be read as a generation — requiring two
        // is what stops "Navi 3" or a future "Navi 4" prototype string matching.
        assert_eq!(generation_from_name("Navi 3"), None);
    }

    #[test]
    fn pci_ids_lookup_finds_a_device_under_its_vendor() {
        assert_eq!(
            pci_ids_lookup(PCI_IDS_FIXTURE, "1002", "744c").as_deref(),
            Some("Navi 31 [Radeon RX 7900 XT/7900 XTX/7900 GRE/7900M]")
        );
        assert_eq!(
            pci_ids_lookup(PCI_IDS_FIXTURE, "1002", "7550").as_deref(),
            Some("Navi 48 [Radeon RX 9070/9070 XT]")
        );
        // sysfs reports the id lowercase; pci.ids is lowercase too, but the
        // comparison must not depend on either.
        assert_eq!(
            pci_ids_lookup(PCI_IDS_FIXTURE, "1002", "744C").as_deref(),
            Some("Navi 31 [Radeon RX 7900 XT/7900 XTX/7900 GRE/7900M]")
        );
    }

    #[test]
    fn pci_ids_lookup_respects_the_vendor_block() {
        // 2684 exists, but under NVIDIA — a scan that ignored the vendor block
        // would happily return an RTX 4090 for an AMD device id.
        assert_eq!(pci_ids_lookup(PCI_IDS_FIXTURE, "1002", "2684"), None);
        // A subsystem line (two tabs) is a board vendor, not a chip: its leading
        // token `1002` must never be mistaken for a device id.
        assert_eq!(pci_ids_lookup(PCI_IDS_FIXTURE, "1002", "1002"), None);
        assert_eq!(pci_ids_lookup(PCI_IDS_FIXTURE, "1002", "dead"), None);
        assert_eq!(pci_ids_lookup("", "1002", "744c"), None);
    }

    #[test]
    fn llm_context_includes_summary_and_omits_empty_fields() {
        let hw = Hardware { amd: true, wayland: true, ..Default::default() };
        assert_eq!(hw.llm_context(), hw.summary());
    }

    #[test]
    fn llm_context_appends_populated_fields_in_order() {
        let hw = Hardware {
            amd: true,
            wayland: true,
            distro: "CachyOS Linux".into(),
            kernel: "6.11.0-2-cachyos".into(),
            cpu_model: "AMD Ryzen 5 9600X".into(),
            ram_gb: 32,
            ..Default::default()
        };
        let ctx = hw.llm_context();
        let expected = format!(
            "{}\nDistro: CachyOS Linux\nKernel: 6.11.0-2-cachyos\nCPU: AMD Ryzen 5 9600X\nRAM: 32 GB",
            hw.summary()
        );
        assert_eq!(ctx, expected);
    }
}
