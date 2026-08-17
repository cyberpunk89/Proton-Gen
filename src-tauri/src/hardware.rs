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

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Hardware {
    pub nvidia: bool,
    pub amd: bool,
    pub intel: bool,
    pub wayland: bool,
    pub kde: bool,
    pub ntsync: bool,
}

fn module_loaded(name: &str) -> bool {
    Path::new("/sys/module").join(name).is_dir()
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
}
