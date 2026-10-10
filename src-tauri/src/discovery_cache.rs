//! The last discovery pass, persisted to `$XDG_CACHE_HOME/protongen/
//! discovery.json` so the next launch can paint the library at once and
//! re-scan behind it (stale-while-revalidate), instead of showing a spinner
//! for a cold-cache walk of every Steam library, VDF and Heroic store file.
//!
//! App-owned and disposable, like the art cache: nothing outside protongen's
//! own cache directory is written, and a missing, corrupt or mismatched file
//! simply means "scan as before". It is never trusted for long — `bootstrap`
//! serves it only to the first frame, and the frontend's background `rescan`
//! replaces it within the same second.

use serde::{Deserialize, Serialize};

use crate::fsutil;
use crate::ipc::Discovery;
use crate::store::Paths;

/// Bumped when [`Discovery`]'s shape changes in a way old files can't decode
/// into. The app version is part of the key too, so this matters mainly for
/// dev builds.
const FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Envelope {
    format: u32,
    app: String,
    /// The Settings → Paths the scan ran with. Different paths are a different
    /// library, so a cache written under other paths is not this one.
    paths: String,
    discovery: Discovery,
}

fn file() -> Option<std::path::PathBuf> {
    Some(fsutil::cache_dir()?.join("discovery.json"))
}

fn paths_key(paths: &Paths) -> String {
    serde_json::to_string(paths).unwrap_or_default()
}

/// The cached discovery for these `paths`, if there is a usable one.
pub(crate) fn load(paths: &Paths) -> Option<Discovery> {
    let text = std::fs::read_to_string(file()?).ok()?;
    decode(&text, paths)
}

fn decode(text: &str, paths: &Paths) -> Option<Discovery> {
    let env: Envelope = serde_json::from_str(text).ok()?;
    (env.format == FORMAT && env.app == env!("CARGO_PKG_VERSION") && env.paths == paths_key(paths))
        .then_some(env.discovery)
}

fn encode(d: &Discovery, paths: &Paths) -> Option<String> {
    serde_json::to_string(&Envelope {
        format: FORMAT,
        app: env!("CARGO_PKG_VERSION").to_string(),
        paths: paths_key(paths),
        discovery: d.clone(),
    })
    .ok()
}

/// Best-effort: a cache that can't be written only costs the next launch its
/// head start, so failures are logged and otherwise ignored.
pub(crate) fn save(d: &Discovery, paths: &Paths) {
    let (Some(path), Some(json)) = (file(), encode(d, paths)) else { return };
    if let Err(e) = fsutil::write_atomic(&path, json.as_bytes()) {
        eprintln!("protongen: couldn't write the discovery cache: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Discovery {
        crate::ipc::scan_discovery(&crate::params::Catalog::bundled(), &Paths::default())
    }

    #[test]
    fn round_trips_under_the_same_paths() {
        let d = sample();
        let paths = Paths::default();
        let back = decode(&encode(&d, &paths).unwrap(), &paths).expect("decodes");
        assert_eq!(back.games.len(), d.games.len());
        assert_eq!(back.runtimes.len(), d.runtimes.len());
        assert_eq!(back.steam_root, d.steam_root);
    }

    #[test]
    fn other_paths_or_garbage_miss() {
        let d = sample();
        let text = encode(&d, &Paths::default()).unwrap();
        let other = Paths { steam_libraries: vec!["/mnt/games".into()], ..Default::default() };
        assert!(decode(&text, &other).is_none());
        assert!(decode("{not json", &Paths::default()).is_none());
        assert!(decode(&text.replace("\"format\":1", "\"format\":0"), &Paths::default()).is_none());
    }
}
