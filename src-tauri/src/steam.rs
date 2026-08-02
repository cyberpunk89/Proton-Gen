//! Resolve the native Steam installation (read-only).

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use steamlocate::SteamDir;

use crate::params::ConfigWarning;

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

/// A real install has a `libraryfolders.vdf` in one of two places.
fn looks_like_steam(path: &Path) -> bool {
    path.join("steamapps/libraryfolders.vdf").is_file()
        || path.join("config/libraryfolders.vdf").is_file()
}

/// Locate the native Steam install. Deliberately ignores the Flatpak install.
///
/// `extra_roots` (from Settings) are tried **before** the built-in candidates: a
/// user only adds one because the defaults were wrong, so an explicit choice
/// outranks a lucky guess. A configured root that isn't a Steam install produces
/// a warning and the search continues — one typo must not kill discovery.
/// Built-in candidates never warn; their absence is the normal case.
pub fn locate_native(extra_roots: &[String], warn: &mut Vec<ConfigWarning>) -> Result<SteamDir> {
    let extra: Vec<PathBuf> = crate::store::Paths::clean(extra_roots)
        .into_iter()
        .map(PathBuf::from)
        .collect();

    for path in &extra {
        if !looks_like_steam(path) {
            warn.push(ConfigWarning::path(
                "Steam root",
                path.display(),
                "no steamapps/libraryfolders.vdf or config/libraryfolders.vdf here",
            ));
            continue;
        }
        match SteamDir::from_dir(path) {
            Ok(dir) => return Ok(dir),
            Err(e) => warn.push(ConfigWarning::path("Steam root", path.display(), e.to_string())),
        }
    }

    for path in candidates() {
        if looks_like_steam(&path) {
            if let Ok(dir) = SteamDir::from_dir(&path) {
                return Ok(dir);
            }
        }
    }

    let tried = candidates()
        .iter()
        .chain(&extra)
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    anyhow::bail!(
        "No native Steam install found. Tried: {tried}. \
         Add yours under Settings → Paths if it lives somewhere else. \
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configured_root_that_is_not_a_steam_install_warns_and_is_skipped() {
        // Assert on the *warning*, not on an Err: `candidates()` may well find
        // the developer's real Steam, and the point of the change is that one
        // bad entry is reported rather than swallowed or fatal.
        let bogus = std::env::temp_dir().join(format!("protongen-not-steam-{}", std::process::id()));
        std::fs::create_dir_all(&bogus).unwrap();

        let mut warn = Vec::new();
        let _ = locate_native(&[bogus.display().to_string()], &mut warn);

        assert_eq!(warn.len(), 1, "the configured root should warn exactly once");
        assert_eq!(warn[0].kind, crate::params::WarningKind::Path);
        assert_eq!(warn[0].file, "Steam root");
        assert_eq!(warn[0].path, bogus.display().to_string());

        std::fs::remove_dir_all(&bogus).ok();
    }

    #[test]
    fn blank_configured_roots_are_ignored_without_warning() {
        // The Settings rows send blanks while the user types; those are not
        // mistakes to report.
        let mut warn = Vec::new();
        let _ = locate_native(&[String::new(), "   ".to_string()], &mut warn);
        assert!(warn.is_empty(), "got: {warn:?}");
    }
}
