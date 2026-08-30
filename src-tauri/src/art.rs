//! Resolve game artwork (portrait capsule, hero, header) from the local Steam
//! cache, with an optional CDN fallback — plus Heroic sideloads, whose art
//! comes from a `file://`/URL hint the frontend hands back (see `fetch`'s
//! `hint` parameter) rather than any cache this module can key by app_id.
//! Read-only; downloaded art is cached under `$XDG_CACHE_HOME/protongen/art`
//! so repeat lookups stay offline.
//!
//! Returns a `data:` URL (base64) so the frontend can drop it straight into an
//! `<img src>` without any asset-protocol/capability configuration.

use std::path::{Path, PathBuf};

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

fn cache_path(app_id: u32, source: &str, kind: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join(format!("protongen/art/{source}_{app_id}_{kind}.img")))
}

fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        _ => "image/jpeg",
    }
}

fn to_data_url(bytes: &[u8], mime: &str) -> String {
    format!("data:{mime};base64,{}", base64_encode(bytes))
}

/// A Heroic `art_cover`/`art_square` hint that is a local file, read straight
/// off disk. Heroic stores these as `file://` URIs; a bare path is accepted
/// too since nothing guarantees the scheme survived whatever wrote it.
fn read_local_hint(hint: &str) -> Option<Vec<u8>> {
    let path = hint.strip_prefix("file://").unwrap_or(hint);
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(bytes)
}

/// Resolve a game's art to a `data:` URL: local Steam cache → previously
/// downloaded cache → (if `online`) Steam CDN / Heroic's own art hint. `None`
/// when nothing is found.
///
/// `hint` is a Heroic sideload's `art_cover`/`art_square` (a `file://` path or
/// a remote URL, e.g. SteamGridDB) — the only lead on its art, since a
/// sideloaded game has no Steam appid a cache lookup could key off. Ignored
/// for every other source.
pub fn fetch(
    steam_root: Option<String>,
    app_id: u32,
    source: &str,
    kind: &str,
    online: bool,
    hint: Option<String>,
) -> Option<String> {
    let hint = hint.as_deref().map(str::trim).filter(|s| !s.is_empty());

    // 1) Local Steam cache / custom grid art, or a Heroic hint that's already
    // a local file — no need to wait for the online step below.
    if source == "heroic" {
        if let Some(h) = hint {
            if !h.starts_with("http://") && !h.starts_with("https://") {
                if let Some(bytes) = read_local_hint(h) {
                    return Some(to_data_url(&bytes, mime_for(Path::new(h))));
                }
            }
        }
    } else if let Some(root) = steam_root.as_deref().map(Path::new) {
        for cand in local_candidates(root, app_id, source, kind) {
            if let Ok(bytes) = std::fs::read(&cand) {
                if !bytes.is_empty() {
                    return Some(to_data_url(&bytes, mime_for(&cand)));
                }
            }
        }
    }

    // 2) Previously downloaded art (kept across runs).
    let cached = cache_path(app_id, source, kind);
    if let Some(cp) = &cached {
        if let Ok(bytes) = std::fs::read(cp) {
            if !bytes.is_empty() {
                return Some(to_data_url(&bytes, "image/jpeg"));
            }
        }
    }

    // 3) Online fallback: the Steam CDN for Steam apps, or a Heroic hint URL.
    if online {
        let remote_url = if source == "steam" {
            Some(cdn_url(app_id, kind))
        } else if source == "heroic" {
            hint.filter(|h| h.starts_with("http://") || h.starts_with("https://"))
                .map(str::to_string)
        } else {
            None
        };
        if let Some(url) = remote_url {
            let req = ehttp::Request::get(url);
            if let Ok(resp) = ehttp::fetch_blocking(&req) {
                if resp.ok && !resp.bytes.is_empty() {
                    if let Some(cp) = &cached {
                        if let Some(parent) = cp.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = std::fs::write(cp, &resp.bytes);
                    }
                    return Some(to_data_url(&resp.bytes, "image/jpeg"));
                }
            }
        }
    }

    None
}

/// Minimal standard-alphabet base64 (avoids pulling in a crate for a few imgs).
fn base64_encode(input: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(A[((n >> 18) & 63) as usize] as char);
        out.push(A[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            A[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            A[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
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
    fn read_local_hint_accepts_file_scheme_and_bare_path() {
        let path = temp_file("cover.jpg", b"fake-jpeg-bytes");
        let uri = format!("file://{}", path.display());
        assert_eq!(read_local_hint(&uri).as_deref(), Some(&b"fake-jpeg-bytes"[..]));
        assert_eq!(
            read_local_hint(&path.display().to_string()).as_deref(),
            Some(&b"fake-jpeg-bytes"[..])
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn read_local_hint_missing_file_is_none_not_a_panic() {
        assert_eq!(read_local_hint("file:///no/such/protongen-test-file.jpg"), None);
    }

    #[test]
    fn heroic_fetch_reads_a_local_file_hint_with_no_network() {
        let path = temp_file("hero-cover.png", b"\x89PNG-fake");
        let uri = format!("file://{}", path.display());
        let url = fetch(None, 0x8000_0001, "heroic", "portrait", false, Some(uri));
        assert_eq!(url.as_deref(), Some("data:image/png;base64,iVBORy1mYWtl"));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn heroic_fetch_with_no_hint_and_offline_finds_nothing() {
        assert_eq!(fetch(None, 0x8000_0002, "heroic", "portrait", false, None), None);
    }
}
