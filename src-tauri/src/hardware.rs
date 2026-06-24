//! Best-effort, read-only hardware/session detection used to mark options as
//! relevant to this machine. Never blocks toggling — unknown is treated as
//! relevant.

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
    /// If an option (with optional gpu hint + needs tags) is **not** relevant to
    /// this machine, return a short reason; otherwise `None`.
    pub fn irrelevance(&self, gpu: Option<&str>, needs: &[String]) -> Option<String> {
        if let Some(g) = gpu {
            let (ok, label) = match g.to_lowercase().as_str() {
                "nvidia" => (self.nvidia, "NVIDIA GPU"),
                "amd" => (self.amd, "AMD GPU"),
                "intel" => (self.intel, "Intel GPU"),
                _ => (true, ""),
            };
            if !ok {
                return Some(format!("needs {label}"));
            }
        }
        for n in needs {
            let (ok, label) = match n.as_str() {
                "wayland" => (self.wayland, "Wayland session"),
                "kde" => (self.kde, "KDE Plasma"),
                "ntsync" => (self.ntsync, "/dev/ntsync"),
                _ => (true, ""),
            };
            if !ok {
                return Some(format!("needs {label}"));
            }
        }
        None
    }

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
