//! Resolve game artwork (portrait capsule, hero, header) from the local Steam
//! cache, with an optional CDN fallback — plus Heroic sideloads, whose art
//! comes from a `file://`/URL hint the frontend hands back (see `fetch`'s
//! `hint` parameter) rather than any cache this module can key by app_id.
//! Read-only; downloaded art is cached under `$XDG_CACHE_HOME/protongen/art`
//! so repeat lookups stay offline.
//!
//! Returns the image's *path*; `ipc::game_art` registers it under an opaque
//! key and the webview loads `art://localhost/<key>`, served by [`response`]
//! from a worker thread. (Tauri's own asset protocol reads files on the UI
//! thread on Linux, uncached — a library grid of tiles made the app stutter.)

use std::path::{Path, PathBuf};

/// The `source` values `fetch` accepts — `GameDto::source`'s vocabulary.
pub const SOURCES: &[&str] = &["steam", "non-steam", "heroic", "nexus"];
/// The `kind` values `fetch` accepts.
pub const KINDS: &[&str] = &["portrait", "hero", "header"];
/// Largest art file read or downloaded. Box art is well under this; anything
/// bigger is not an image worth base64-ing into the webview.
const MAX_ART_BYTES: u64 = 16 * 1024 * 1024;
/// Per-request timeout for remote art. Shorter than ehttp's 30 s default: the
/// frontend runs a dozen of these at once, and offline each would otherwise
/// pin a slot for the full default.
const ART_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// How long a remote 4xx is trusted before asking again — art does get added
/// to the CDN after release, just not often.
const MISS_TTL_SECS: u64 = 7 * 24 * 60 * 60;

/// Candidate local file paths for a game's art, in priority order.
fn local_candidates(steam_root: &Path, app_id: u32, source: &str, kind: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();

    if source == "non-steam" {
        // Custom grid art lives per-user under userdata/<id>/config/grid.
        let suffixes: &[&str] = match kind {
            "portrait" => &["p.jpg", "p.png"],
            "hero" => &["_hero.jpg", "_hero.png"],
            _ => &[".jpg", ".png"], // header / landscape capsule
        };
        if let Ok(users) = std::fs::read_dir(steam_root.join("userdata")) {
            for u in users.flatten() {
                let grid = u.path().join("config/grid");
                for sfx in suffixes {
                    v.push(grid.join(format!("{app_id}{sfx}")));
                }
            }
        }
        return v;
    }

    // Steam games: appcache/librarycache — flat (older) + per-appid subdir (newer).
    let cache = steam_root.join("appcache/librarycache");
    let (flat, sub): (&[&str], &[&str]) = match kind {
        "portrait" => (
            &["_library_600x900.jpg"],
            &["library_600x900.jpg", "library_600x900.png"],
        ),
        "hero" => (
            &["_library_hero.jpg"],
            &["library_hero.jpg", "library_hero.png"],
        ),
        _ => (&["_header.jpg"], &["header.jpg", "header.png"]),
    };
    for f in flat {
        v.push(cache.join(format!("{app_id}{f}")));
    }
    for s in sub {
        v.push(cache.join(app_id.to_string()).join(s));
    }
    v
}

/// Steam CDN URL for a Steam app's art (no art exists there for shortcuts).
fn cdn_url(app_id: u32, kind: &str) -> String {
    let file = match kind {
        "portrait" => "library_600x900.jpg",
        "hero" => "library_hero.jpg",
        _ => "header.jpg",
    };
    format!("https://steamcdn-a.akamaihd.net/steam/apps/{app_id}/{file}")
}

/// The cache file's path *without* its extension; the extension comes from
/// what the downloaded bytes are (see [`image_ext`]), so the asset protocol
/// can serve the right content type.
fn cache_stem(app_id: u32, source: &str, kind: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join(format!("protongen/art/{source}_{app_id}_{kind}")))
}

/// What image format `bytes` are, by magic number; `None` for anything that
/// isn't one (an HTML error page served with a 200, say).
fn image_ext(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0xFF, 0xD8, 0xFF, ..] => Some("jpg"),
        [0x89, b'P', b'N', b'G', ..] => Some("png"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("webp"),
        [b'G', b'I', b'F', b'8', ..] => Some("gif"),
        [0, 0, 1, 0, ..] => Some("ico"),
        _ => None,
    }
}

/// A previously downloaded image for `stem`, if any. A pre-asset-protocol
/// `.img` file is renamed to its real extension on the way.
fn cached_file(stem: &Path) -> Option<PathBuf> {
    for ext in ["jpg", "png", "webp", "gif", "ico"] {
        let p = stem.with_extension(ext);
        if usable(&p) {
            return Some(p);
        }
    }
    let legacy = stem.with_extension("img");
    if usable(&legacy) {
        let head = std::fs::read(&legacy).ok()?;
        let target = stem.with_extension(image_ext(&head)?);
        return std::fs::rename(&legacy, &target).ok().map(|_| target);
    }
    None
}

/// A non-empty regular file no bigger than [`MAX_ART_BYTES`].
fn usable(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() > 0 && m.len() <= MAX_ART_BYTES)
}

/// Sidecar recording when the remote source last said this art doesn't exist.
fn miss_marker(stem: &Path) -> PathBuf {
    stem.with_extension("miss")
}

/// Whether `marker` holds a timestamp newer than `MISS_TTL_SECS` before `now`.
/// A missing or unreadable marker is "not missed", so the worst case is one
/// extra request.
fn recent_miss(marker: &Path, now: u64) -> bool {
    std::fs::read_to_string(marker)
        .ok()
        .and_then(|t| t.trim().parse::<u64>().ok())
        .is_some_and(|ts| now.saturating_sub(ts) < MISS_TTL_SECS)
}

/// A Heroic `art_cover`/`art_square` hint that is a local file. Heroic stores
/// these as `file://` URIs; a bare path is accepted too since nothing
/// guarantees the scheme survived whatever wrote it.
fn local_hint(hint: &str) -> Option<PathBuf> {
    let p = PathBuf::from(hint.strip_prefix("file://").unwrap_or(hint));
    usable(&p).then_some(p)
}

/// Resolve a game's art to an image file: local Steam cache → previously
/// downloaded cache → (if `online`) Steam CDN / Heroic's own art hint,
/// downloaded into the cache. `None` when nothing is found.
///
/// `hint` is a Heroic sideload's `art_cover`/`art_square` (a `file://` path or
/// a remote URL, e.g. SteamGridDB), or a Nexus game's artwork folder — the
/// only lead on their art, since neither has a Steam appid a cache lookup
/// could key off. Ignored for every other source.
pub fn fetch(
    steam_root: Option<String>,
    app_id: u32,
    source: &str,
    kind: &str,
    online: bool,
    hint: Option<String>,
) -> Option<PathBuf> {
    let hint = hint.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let local = |src: &str| {
        steam_root
            .as_deref()
            .map(Path::new)
            .into_iter()
            .flat_map(|root| local_candidates(root, app_id, src, kind))
            .find(|c| usable(c))
    };

    // 1) Local art — no need to wait for the online step below.
    match source {
        // Nexus keeps art per game as `<dir>/{cover,hero,wide}.<ext>`; failing
        // that, its Steam shortcut (same appid) may carry custom grid art.
        "nexus" => {
            return hint
                .and_then(|d| crate::nexus::art_file(d, kind))
                .filter(|f| usable(f))
                .or_else(|| local("non-steam"));
        }
        "heroic" => {
            if let Some(f) = hint.filter(|h| !h.starts_with("http://") && !h.starts_with("https://")).and_then(local_hint) {
                return Some(f);
            }
        }
        _ => {
            if let Some(f) = local(source) {
                return Some(f);
            }
        }
    }

    // 2) Previously downloaded art (kept across runs).
    let stem = cache_stem(app_id, source, kind)?;
    if let Some(f) = cached_file(&stem) {
        return Some(f);
    }

    // 3) Online fallback: the Steam CDN for Steam apps, or a Heroic hint URL.
    if !online {
        return None;
    }
    let remote_url = match source {
        "steam" => Some(cdn_url(app_id, kind)),
        "heroic" => hint.filter(|h| h.starts_with("http://") || h.starts_with("https://")).map(str::to_string),
        _ => None,
    }?;
    let miss = miss_marker(&stem);
    if recent_miss(&miss, crate::fsutil::unix_ts()) {
        return None;
    }
    let req = ehttp::Request::get(remote_url).with_timeout(Some(ART_TIMEOUT));
    match ehttp::fetch_blocking(&req) {
        Ok(resp) if resp.ok && resp.bytes.len() as u64 <= MAX_ART_BYTES => {
            // Only something that is an image gets cached and served; the
            // extension it gets is what the asset protocol's content type
            // comes from. Atomic, so a torn write can't be served forever.
            let ext = image_ext(&resp.bytes)?;
            let file = stem.with_extension(ext);
            crate::fsutil::write_atomic(&file, &resp.bytes).ok()?;
            Some(file)
        }
        // A definite "no such art" (404 and friends): remember it so the
        // next session doesn't spend a request slot asking again.
        // Network errors are not remembered — offline is not "missing".
        Ok(resp) if (400..500).contains(&resp.status) => {
            let _ = crate::fsutil::write_atomic(&miss, crate::fsutil::unix_ts().to_string().as_bytes());
            None
        }
        _ => None,
    }
}

/// The key a resolved art file is served under: which art it is, plus the
/// file's mtime so a replaced image gets a new (uncached) URL. URL-safe by
/// construction — `source`/`kind` are from fixed lists.
pub fn key_for(app_id: u32, source: &str, kind: &str, path: &Path) -> String {
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    format!("{source}-{app_id}-{kind}-{mtime}")
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        _ => "image/jpeg",
    }
}

/// The `art` protocol's response for a registered file (`None`: unknown key).
/// Immutable caching is safe because the key changes with the file's mtime.
pub fn response(path: Option<&Path>) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{header, Response, StatusCode};
    let body = path.filter(|p| usable(p)).and_then(|p| std::fs::read(p).ok().map(|b| (p, b)));
    match body {
        Some((p, bytes)) => Response::builder()
            .header(header::CONTENT_TYPE, content_type(p))
            .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
            .body(bytes),
        None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()),
    }
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_ext_reads_magic_numbers() {
        assert_eq!(image_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(image_ext(b"\x89PNG\r\n"), Some("png"));
        assert_eq!(image_ext(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(image_ext(b"<!doctype html>"), None, "an error page is not art");
        assert_eq!(image_ext(b""), None);
    }

    #[test]
    fn a_legacy_img_cache_file_gets_its_real_extension() {
        let dir = std::env::temp_dir().join(format!("protongen-art-legacy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stem = dir.join("steam_10_hero");
        std::fs::write(stem.with_extension("img"), b"\x89PNG-ish").unwrap();
        assert_eq!(cached_file(&stem), Some(stem.with_extension("png")));
        assert!(!stem.with_extension("img").exists());
        assert_eq!(cached_file(&stem), Some(stem.with_extension("png")), "found directly next time");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn steam_portrait_candidates_include_flat_and_subdir() {
        let root = Path::new("/steam");
        let c = local_candidates(root, 553850, "steam", "portrait");
        assert!(c.contains(&root.join("appcache/librarycache/553850_library_600x900.jpg")));
        assert!(c.contains(&root.join("appcache/librarycache/553850/library_600x900.jpg")));
    }

    #[test]
    fn nonsteam_uses_userdata_grid() {
        // No userdata dir under a bogus root → no candidates, no panic.
        let c = local_candidates(Path::new("/nope"), 42, "non-steam", "portrait");
        assert!(c.is_empty());
    }

    /// A process-unique temp file, since this crate takes no dev-dependency on
    /// a tempfile crate.
    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("protongen-art-test-{ts}-{name}"));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn local_hint_accepts_file_scheme_and_bare_path() {
        let path = temp_file("cover.jpg", b"fake-jpeg-bytes");
        let uri = format!("file://{}", path.display());
        assert_eq!(local_hint(&uri), Some(path.clone()));
        assert_eq!(local_hint(&path.display().to_string()), Some(path.clone()));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn local_hint_missing_or_empty_file_is_none() {
        assert_eq!(local_hint("file:///no/such/protongen-test-file.jpg"), None);
        let empty = temp_file("empty.png", b"");
        assert_eq!(local_hint(&empty.display().to_string()), None);
        std::fs::remove_file(&empty).ok();
    }

    #[test]
    fn heroic_fetch_returns_a_local_file_hint_with_no_network() {
        let path = temp_file("hero-cover.png", b"\x89PNG-fake");
        let uri = format!("file://{}", path.display());
        assert_eq!(fetch(None, 0x8000_0001, "heroic", "portrait", false, Some(uri)), Some(path.clone()));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn heroic_fetch_with_no_hint_and_offline_finds_nothing() {
        assert_eq!(fetch(None, 0x8000_0002, "heroic", "portrait", false, None), None);
    }

    #[test]
    fn a_fresh_miss_marker_suppresses_the_refetch_and_an_old_one_does_not() {
        let now = 2_000_000_000;
        let fresh = temp_file("fresh.miss", (now - 60).to_string().as_bytes());
        let stale = temp_file("stale.miss", (now - MISS_TTL_SECS - 1).to_string().as_bytes());
        let junk = temp_file("junk.miss", b"not a number");
        assert!(recent_miss(&fresh, now));
        assert!(!recent_miss(&stale, now));
        assert!(!recent_miss(&junk, now), "an unreadable marker must not block art forever");
        assert!(!recent_miss(Path::new("/no/such/protongen.miss"), now));
        for p in [fresh, stale, junk] {
            std::fs::remove_file(p).ok();
        }
    }

    #[test]
    fn miss_marker_sits_next_to_the_cached_image() {
        let stem = Path::new("/c/protongen/art/steam_10_hero");
        assert_eq!(miss_marker(stem), Path::new("/c/protongen/art/steam_10_hero.miss"));
    }

    #[test]
    fn art_protocol_serves_registered_files_with_a_long_cache() {
        let path = temp_file("served.png", b"\x89PNG-bytes");
        let ok = response(Some(&path));
        assert_eq!(ok.status(), 200);
        assert_eq!(ok.headers()["content-type"], "image/png");
        assert!(ok.headers()["cache-control"].to_str().unwrap().contains("immutable"));
        assert_eq!(ok.body(), b"\x89PNG-bytes");
        assert_eq!(response(None).status(), 404);
        assert_eq!(response(Some(Path::new("/no/such.png"))).status(), 404);

        let key = key_for(7, "steam", "portrait", &path);
        assert!(key.starts_with("steam-7-portrait-"));
        assert!(key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        std::fs::remove_file(&path).ok();
    }
}
