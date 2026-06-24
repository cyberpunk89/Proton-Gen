//! Resolve the native Steam installation (read-only).

use anyhow::{Context, Result};
use std::path::PathBuf;
use steamlocate::SteamDir;

/// Candidate native Steam roots, in priority order.
fn candidates() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut v = Vec::new();
    if let Some(h) = home {
        v.push(h.join(".local/share/Steam"));
        v.push(h.join(".steam/steam"));
        v.push(h.join(".steam/root"));
    }
    v
}

/// Locate the native Steam install. Deliberately ignores the Flatpak install.
pub fn locate_native() -> Result<SteamDir> {
    for path in candidates() {
        // Require a real install: libraryfolders.vdf must be present.
        if path.join("steamapps/libraryfolders.vdf").is_file()
            || path.join("config/libraryfolders.vdf").is_file()
        {
            if let Ok(dir) = SteamDir::from_dir(&path) {
                return Ok(dir);
            }
        }
    }
    anyhow::bail!(
        "No native Steam install found under ~/.local/share/Steam or ~/.steam. \
         (Flatpak Steam is intentionally not used.)"
    );
}

/// Path to the Steam root, as a display string.
pub fn root_display(dir: &SteamDir) -> String {
    dir.path().display().to_string()
}

/// Resolve the user-level compatibilitytools.d directory for this install.
pub fn user_compat_tools_dir(dir: &SteamDir) -> PathBuf {
    dir.path().join("compatibilitytools.d")
}

/// Bundled Valve runtimes live under steamapps/common.
pub fn common_dir(dir: &SteamDir) -> Result<PathBuf> {
    let p = dir.path().join("steamapps/common");
    p.canonicalize()
        .with_context(|| format!("steamapps/common not found at {}", p.display()))
}
