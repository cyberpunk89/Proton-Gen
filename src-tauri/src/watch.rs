//! Live refresh: notice when Steam, Heroic or Nexus change the files discovery
//! reads, and tell the frontend, so "applied in Steam" and a newly installed
//! game show up without waiting for the window to regain focus.
//!
//! Deliberately a poll of a few dozen `stat`s every [`TICK`] rather than
//! inotify via a new dependency: the set is small (two Steam config files per
//! user, one directory per library, a handful of Heroic/Nexus files), the
//! files are often replaced by rename (which a plain inotify file watch
//! misses), and a stat loop has no failure modes worth handling. Read-only,
//! like every other discovery path.
//!
//! Two groups, two events, because they cost very differently to act on:
//! - [`STEAM_CONFIG_EVENT`] — `localconfig.vdf` / `config.vdf` changed: the
//!   frontend re-reads just launch options + compat tools (cheap).
//! - [`LIBRARY_EVENT`] — an appmanifest appeared/disappeared, shortcuts.vdf, a
//!   Heroic store cache or Nexus's games.json changed: the frontend rescans.
//!   Rate-limited to one per [`LIBRARY_MIN_GAP`], since Steam rewrites
//!   appmanifests continuously while a download runs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use tauri::{AppHandle, Emitter, Manager};

use crate::store::Paths;

pub const STEAM_CONFIG_EVENT: &str = "steam-config-changed";
pub const LIBRARY_EVENT: &str = "library-changed";

const TICK: Duration = Duration::from_millis(1500);
const LIBRARY_MIN_GAP: Duration = Duration::from_secs(5);
/// Re-derive the watch list this often anyway: a new Steam user or library
/// folder adds paths that a stale list would never look at.
const REBUILD_EVERY: u32 = 40;

#[derive(Default, Debug)]
struct WatchSet {
    steam_config: Vec<PathBuf>,
    library: Vec<PathBuf>,
}

/// mtime + size; `None` when the path doesn't exist (so appearing and
/// disappearing both count as a change).
type Stamp = Option<(SystemTime, u64)>;

fn stamp(p: &PathBuf) -> Stamp {
    let m = std::fs::metadata(p).ok()?;
    Some((m.modified().ok()?, m.len()))
}

fn build(paths: &Paths) -> WatchSet {
    let mut w = WatchSet::default();
    if let Ok(dir) = crate::steam::locate_native(&paths.steam_roots, &mut Vec::new()) {
        let root = dir.path();
        w.steam_config.push(root.join("config/config.vdf"));
        if let Ok(users) = std::fs::read_dir(root.join("userdata")) {
            for user in users.flatten() {
                let cfg = user.path().join("config");
                w.steam_config.push(cfg.join("localconfig.vdf"));
                w.library.push(cfg.join("shortcuts.vdf"));
            }
        }
        w.library.push(root.join("config/libraryfolders.vdf"));
        w.library.push(root.join("steamapps/libraryfolders.vdf"));
        if let Ok(libs) = dir.libraries() {
            // The directory, not each manifest: its mtime moves when a
            // manifest is added, removed or replaced by rename.
            w.library.extend(libs.flatten().map(|l| l.path().join("steamapps")));
        }
    }
    for extra in Paths::clean(&paths.steam_libraries) {
        w.library.push(PathBuf::from(extra).join("steamapps"));
    }
    w.library.extend(crate::heroic::watched_files());
    w.library.extend(crate::nexus::registry_path());
    w
}

fn snapshot(paths: &[PathBuf]) -> HashMap<PathBuf, Stamp> {
    paths.iter().map(|p| (p.clone(), stamp(p))).collect()
}

/// Whether any path watched in both snapshots changed. Paths only in one of
/// them were added to / dropped from the watch list, which is not a change.
fn changed(old: &HashMap<PathBuf, Stamp>, new: &HashMap<PathBuf, Stamp>) -> bool {
    new.iter().any(|(p, s)| old.get(p).is_some_and(|o| o != s))
}

fn paths_key(p: &Paths) -> String {
    serde_json::to_string(p).unwrap_or_default()
}

/// Start the watcher thread. It lives as long as the app.
pub fn spawn(app: AppHandle) {
    let spawned = std::thread::Builder::new().name("protongen-watch".into()).spawn(move || {
        let current = || app.state::<crate::ipc::AppState>().paths();
        let mut paths = current();
        let mut key = paths_key(&paths);
        let mut set = build(&paths);
        let mut cfg_seen = snapshot(&set.steam_config);
        let mut lib_seen = snapshot(&set.library);
        let mut library_pending = false;
        let mut last_library_emit = Instant::now() - LIBRARY_MIN_GAP;
        let mut ticks = 0u32;

        loop {
            std::thread::sleep(TICK);
            ticks = ticks.wrapping_add(1);

            let fresh = current();
            let fresh_key = paths_key(&fresh);
            if fresh_key != key || ticks % REBUILD_EVERY == 0 {
                paths = fresh;
                key = fresh_key;
                set = build(&paths);
            }

            let cfg_now = snapshot(&set.steam_config);
            if changed(&cfg_seen, &cfg_now) {
                let _ = app.emit(STEAM_CONFIG_EVENT, ());
            }
            cfg_seen = cfg_now;

            let lib_now = snapshot(&set.library);
            library_pending |= changed(&lib_seen, &lib_now);
            lib_seen = lib_now;
            // Trailing-edge rate limit: a burst of changes still ends in one
            // event after it settles past the gap, never in silence.
            if library_pending && last_library_emit.elapsed() >= LIBRARY_MIN_GAP {
                library_pending = false;
                last_library_emit = Instant::now();
                let _ = app.emit(LIBRARY_EVENT, ());
            }
        }
    });
    if let Err(e) = spawned {
        eprintln!("protongen: live refresh disabled, couldn't start the watcher: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_paths_in_both_snapshots_count() {
        let t = SystemTime::UNIX_EPOCH;
        let a = PathBuf::from("/a");
        let b = PathBuf::from("/b");
        let old: HashMap<_, _> = [(a.clone(), Some((t, 1)))].into();
        // Same stamp, plus a newly watched path: no change.
        let same: HashMap<_, _> = [(a.clone(), Some((t, 1))), (b.clone(), None)].into();
        assert!(!changed(&old, &same));
        // Size moved, or the file vanished: change.
        assert!(changed(&old, &[(a.clone(), Some((t, 2)))].into()));
        assert!(changed(&old, &[(a.clone(), None)].into()));
        // A file appearing where there was none: change.
        let none: HashMap<_, _> = [(b.clone(), None)].into();
        assert!(changed(&none, &[(b, Some((t, 0)))].into()));
    }
}
