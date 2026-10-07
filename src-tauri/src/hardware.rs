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
    /// Connected displays and their native mode, for the gamescope builder's
    /// output-size presets. Empty when sysfs has nothing (a VM, no KMS).
    pub monitors: Vec<Monitor>,
}

/// One connected display, from `/sys/class/drm/<card>-<connector>/`.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Monitor {
    /// The DRM connector, e.g. `DP-2`.
    pub connector: String,
    pub width: u32,
    pub height: u32,
    /// Highest refresh the EDID advertises at that resolution, rounded.
    /// `None` when the EDID is missing or has no timing for it.
    pub refresh_hz: Option<u32>,
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
        monitors: detect_monitors(Path::new("/sys/class/drm")),
    }
}

/// `(width, height, refresh Hz)` from one 18-byte EDID detailed timing
/// descriptor, or `None` for a non-timing descriptor (pixel clock 0).
fn edid_timing(d: &[u8]) -> Option<(u32, u32, f64)> {
    if d.len() < 18 {
        return None;
    }
    let clock = u32::from(u16::from_le_bytes([d[0], d[1]])) * 10_000;
    if clock == 0 {
        return None;
    }
    let h_active = u32::from(d[2]) | (u32::from(d[4] & 0xF0) << 4);
    let h_blank = u32::from(d[3]) | (u32::from(d[4] & 0x0F) << 8);
    let v_active = u32::from(d[5]) | (u32::from(d[7] & 0xF0) << 4);
    let v_blank = u32::from(d[6]) | (u32::from(d[7] & 0x0F) << 8);
    let total = (h_active + h_blank) * (v_active + v_blank);
    (total > 0).then(|| (h_active, v_active, f64::from(clock) / f64::from(total)))
}

/// Every detailed timing in an EDID: the four in the base block, plus those
/// in CTA-861 extension blocks (where high-refresh modes usually live — the
/// base block's preferred timing is often just 60 Hz).
fn edid_timings(edid: &[u8]) -> Vec<(u32, u32, f64)> {
    let mut out = Vec::new();
    if edid.len() >= 128 {
        for off in [54, 72, 90, 108] {
            out.extend(edid_timing(&edid[off..off + 18]));
        }
    }
    for ext in edid.chunks(128).skip(1) {
        if ext.len() < 128 || ext[0] != 0x02 {
            continue;
        }
        let mut off = usize::from(ext[2]);
        while off >= 4 && off + 18 <= 127 {
            match edid_timing(&ext[off..off + 18]) {
                Some(t) => out.push(t),
                None => break,
            }
            off += 18;
        }
    }
    out
}

/// Connected displays under `drm` (normally `/sys/class/drm`): the native
/// mode is the first line of `modes`; its refresh comes from the EDID.
fn detect_monitors(drm: &Path) -> Vec<Monitor> {
    let Ok(entries) = std::fs::read_dir(drm) else {
        return Vec::new();
    };
    let mut out: Vec<Monitor> = entries
        .flatten()
        .filter_map(|e| {
            let dir = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            // `card1-DP-2` → `DP-2`; skip `card1`, `renderD128`, `version`.
            let connector = name.split_once('-').map(|(_, c)| c.to_string())?;
            if std::fs::read_to_string(dir.join("status")).ok()?.trim() != "connected" {
                return None;
            }
            let mode = std::fs::read_to_string(dir.join("modes")).ok()?;
            let (w, h) = mode.lines().next()?.trim().split_once('x')?;
            let (width, height) = (w.parse::<u32>().ok()?, h.trim_end_matches('i').parse::<u32>().ok()?);
            let edid = std::fs::read(dir.join("edid")).unwrap_or_default();
            let refresh_hz = edid_timings(&edid)
                .into_iter()
                .filter(|&(tw, th, _)| tw == width && th == height)
                .map(|(_, _, hz)| hz)
                .fold(None, |best: Option<f64>, hz| Some(best.map_or(hz, |b| b.max(hz))))
                .map(|hz| hz.round() as u32);
            Some(Monitor { connector, width, height, refresh_hz })
        })
        .collect();
    out.sort_by(|a, b| a.connector.cmp(&b.connector));
    out
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

    /// An 18-byte detailed timing for `w`x`h` at `clock_10khz`, with the
    /// given blanking — the inverse of `edid_timing`.
    fn dtd(clock_10khz: u16, w: u32, hb: u32, h: u32, vb: u32) -> [u8; 18] {
        let mut d = [0u8; 18];
        d[0..2].copy_from_slice(&clock_10khz.to_le_bytes());
        d[2] = (w & 0xFF) as u8;
        d[3] = (hb & 0xFF) as u8;
        d[4] = (((w >> 8) as u8) << 4) | ((hb >> 8) as u8 & 0x0F);
        d[5] = (h & 0xFF) as u8;
        d[6] = (vb & 0xFF) as u8;
        d[7] = (((h >> 8) as u8) << 4) | ((vb >> 8) as u8 & 0x0F);
        d
    }

    #[test]
    fn edid_timing_decodes_a_real_descriptor() {
        // A real 2560x1440 monitor's base-block preferred timing.
        let (w, h, hz) = edid_timing(&dtd(24150, 2560, 160, 1440, 41)).unwrap();
        assert_eq!((w, h), (2560, 1440));
        assert_eq!(hz.round() as u32, 60);
        assert!(edid_timing(&[0u8; 18]).is_none(), "a display descriptor, not a timing");
    }

    #[test]
    fn monitors_take_the_highest_refresh_at_the_native_mode() {
        let root = std::env::temp_dir().join(format!("protongen-drm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let conn = root.join("card1-DP-2");
        std::fs::create_dir_all(&conn).unwrap();
        std::fs::create_dir_all(root.join("card1-HDMI-A-1")).unwrap();
        std::fs::write(root.join("card1-HDMI-A-1/status"), "disconnected\n").unwrap();
        std::fs::write(conn.join("status"), "connected\n").unwrap();
        std::fs::write(conn.join("modes"), "2560x1440\n1920x1080\n").unwrap();
        let mut edid = vec![0u8; 256];
        edid[54..72].copy_from_slice(&dtd(24150, 2560, 160, 1440, 41)); // 60 Hz
        edid[128] = 0x02;
        edid[130] = 4;
        edid[132..150].copy_from_slice(&dtd(48300, 2560, 160, 1440, 41)); // ~120 Hz
        edid[150..168].copy_from_slice(&dtd(29700, 1920, 280, 1080, 45)); // other mode
        std::fs::write(conn.join("edid"), &edid).unwrap();

        let m = detect_monitors(&root);
        assert_eq!(m, vec![Monitor { connector: "DP-2".into(), width: 2560, height: 1440, refresh_hz: Some(120) }]);
        assert!(detect_monitors(&root.join("nope")).is_empty());
        std::fs::remove_dir_all(&root).ok();
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
