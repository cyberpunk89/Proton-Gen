//! Best-effort, read-only hardware/session detection.
//!
//! Detection only — the relevance *filter* that consumes this lives in
//! `src/lib/util.ts irrelevance()` and has no Rust counterpart. There used to be
//! one here; it went three capability tags stale and rotted into dead code,
//! because `hdr`/`fsr4`/`rdna3`/`rdna4` are opt-in settings held in the frontend
//! store that never reach this side. `lint.rs` is the one Rust consumer, and it
//! reads the fields directly.
//!
//! Note what is *not* detected: GPU architecture. `amdgpu` is loaded for
//! everything from GCN onwards and nothing here reads PCI ids, so the RDNA
//! generation is a user declaration (`store.gpu_gen`), not a fact.

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
        let gpu = if gpus.is_empty() { "unknown GPU".to_string() } else { gpus.join("+") };
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
