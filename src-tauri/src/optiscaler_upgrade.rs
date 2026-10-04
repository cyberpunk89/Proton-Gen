//! Fetch the latest OptiScaler release (stable, or the daily nightly build)
//! from GitHub and extract it into a game's install directory — the manual "grab the newest OptiScaler build
//! and drop it into the game folder" workflow, automated.
//!
//! **A deliberate, narrow exception to the read-only-by-contract invariant**
//! (see `design.md` §11 and the crate-level doc comment in `lib.rs`): this is
//! the only place protongen writes into a *game's own* directory rather than
//! just building a command string. Justified because: (1) the user explicitly
//! asked for exactly this, describing their own existing manual workflow; (2)
//! `update.rs` already does the identical shape of thing (GitHub release
//! fetch → verify → atomic swap) for the app's own binary, so this reuses its
//! `fetch_bytes`/`fetch_text` helpers; (3) it only ever *places files* — it
//! never executes anything, so there's no new code-execution surface; (4) it
//! is gated behind an explicit per-click confirmation that names the exact
//! source URL, version and destination before writing anything, never run
//! automatically.
//!
//! Unlike `update.rs`, OptiScaler's releases publish no checksum to verify
//! against — integrity here rests on HTTPS plus fetching straight from the
//! project's own GitHub Releases API, the same trust boundary the app already
//! extends to it via `PROTON_USE_OPTISCALER`/`PROTON_OPTISCALER_CONFIG`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::update::{DOWNLOAD_TIMEOUT, fetch_bytes_with, fetch_text};

const USER_AGENT: &str = "protongen-optiscaler-upgrade";

/// Which upstream build line to fetch. Stable is the project's tagged
/// releases; nightly is the daily build the project publishes to a separate
/// repo (the `nightly` tag on the main repo is just a changelog pointing
/// there, with no assets).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Stable,
    Nightly,
}

impl Channel {
    fn repo(self) -> &'static str {
        match self {
            Channel::Stable => "optiscaler/OptiScaler",
            Channel::Nightly => "optiscaler/OptiScaler-nightly",
        }
    }
}

/// Files that mark a game folder as already having a manual OptiScaler install.
/// Only a folder with one of these (or an OptiScaler proxy, see
/// [`PROXY_NAMES`]) present is offered the fetch action: the point is to
/// refresh an existing install with a newer upstream build, never to inject
/// OptiScaler into a game that isn't using it.
///
/// CachyOS Proton's own `PROTON_USE_OPTISCALER` never leaves these here: it
/// injects from the prefix (`pfx/drive_c/windows/system32/umu/`, see
/// proton-cachyos `protonfixes/upscalers.py`) and doesn't touch the game
/// folder. So a hit here is always a hand install — and fetching into it with
/// injection on stacks a second OptiScaler (`lint.rs`'s
/// `optiscaler-double-install`).
const MARKER_FILES: &[&str] = &["OptiScaler.dll", "OptiScaler.ini"];

/// The DLL as it ships in the release archive.
const DLL_FILE: &str = "OptiScaler.dll";

/// The names OptiScaler's install guide has you rename [`DLL_FILE`] to, so
/// the game loads it as a proxy. A hand install that did so has no
/// `OptiScaler.dll` at all — extracting the archive verbatim would put the
/// new build *beside* the live one, where nothing loads it.
const PROXY_NAMES: &[&str] = &[
    "dxgi.dll",
    "winmm.dll",
    "version.dll",
    "dbghelp.dll",
    "d3d12.dll",
    "wininet.dll",
    "winhttp.dll",
];

/// The one file the extractor treats specially: never overwritten if the
/// destination already has one, since it may carry tuning applied through
/// the app's own OptiScaler builder (`PROTON_OPTISCALER_CONFIG`).
const INI_FILE: &str = "OptiScaler.ini";

/// Whether an OptiScaler install was found for a game, for the frontend to
/// decide whether to offer the fetch action at all.
#[derive(Clone, Debug, Serialize)]
pub struct OptiscalerStatus {
    pub install_dir: Option<String>,
    pub found: bool,
    /// Proxy-named DLLs ([`PROXY_NAMES`], on-disk spelling) that are OptiScaler
    /// builds: the live entry point the fetch writes the new DLL over. More
    /// than one is an already-stacked install, which the fetch refuses.
    pub proxies: Vec<String>,
    /// An `OptiScaler.dll` sits beside a proxy — nothing loads it; typically
    /// left by a fetch from before this was proxy-aware.
    pub stray_dll: bool,
}

/// Detect an existing manual install in the folder's root. `install_dir` is
/// `None` when nothing could be resolved for this game (see
/// `games::Game::install_dir`).
pub fn detect(install_dir: Option<&Path>) -> OptiscalerStatus {
    let Some(dir) = install_dir else {
        return OptiscalerStatus { install_dir: None, found: false, proxies: Vec::new(), stray_dll: false };
    };
    let names = dir_names(dir);
    let has = |want: &str| names.iter().any(|n| n.eq_ignore_ascii_case(want));
    let proxies = optiscaler_proxies(dir, &names);
    OptiscalerStatus {
        install_dir: Some(dir.display().to_string()),
        found: !proxies.is_empty() || MARKER_FILES.iter().any(|f| has(f)),
        stray_dll: !proxies.is_empty() && has(DLL_FILE),
        proxies,
    }
}

/// File names directly in `dir`; empty when it can't be read.
fn dir_names(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()
        })
        .unwrap_or_default()
}

/// The entries of `names` that carry a proxy name *and* are OptiScaler — a
/// `dxgi.dll` alone is as likely ReShade or DXVK, and `dbghelp.dll` is often
/// the game's own, so the name is never enough to overwrite one.
fn optiscaler_proxies(dir: &Path, names: &[String]) -> Vec<String> {
    PROXY_NAMES
        .iter()
        .filter_map(|p| names.iter().find(|n| n.eq_ignore_ascii_case(p)))
        .filter(|n| is_optiscaler_dll(&dir.join(n)))
        .cloned()
        .collect()
}

/// Whether the DLL at `path` names OptiScaler in its strings (its ini/log file
/// names and `D3D12_OptiScaler` path), as ASCII or UTF-16LE. Unreadable
/// counts as no.
fn is_optiscaler_dll(path: &Path) -> bool {
    const NEEDLE: &[u8] = b"optiscaler";
    let Ok(bytes) = std::fs::read(path) else { return false };
    bytes.windows(NEEDLE.len()).any(|w| w.eq_ignore_ascii_case(NEEDLE))
        || bytes.windows(NEEDLE.len() * 2).any(|w| {
            w.chunks_exact(2).zip(NEEDLE).all(|(c, n)| c[1] == 0 && c[0].eq_ignore_ascii_case(n))
        })
}

/// What the new `OptiScaler.dll` must be written as in `install_dir`: the live
/// proxy's on-disk name, or `None` for the archive's own name when the install
/// isn't renamed. Refuses a folder with two OptiScaler proxies — upgrading one
/// would leave the other loading the old build.
fn entry_point(install_dir: &Path) -> Result<Option<String>, String> {
    let mut proxies = optiscaler_proxies(install_dir, &dir_names(install_dir));
    if proxies.len() > 1 {
        return Err(format!(
            "{} are all OptiScaler — two copies load at once. Remove all but one before upgrading.",
            proxies.join(", ")
        ));
    }
    Ok(proxies.pop())
}

/// What a *manual* OptiScaler install leaves in a game folder (compared
/// case-insensitively). CachyOS Proton's own `PROTON_USE_OPTISCALER` injection
/// lives in the prefix (`system32/umu`) and never writes here — so with it on,
/// any of these means a second OptiScaler with its own FSR/XeSS runtimes, and
/// the two builds end up mixed in one process.
const MANUAL_INSTALL_MARKERS: &[&str] = &[
    "OptiScaler.dll",
    "OptiScaler.ini",
    "OptiScaler.log",
    // Newer archives keep their runtimes in an `OptiScaler/` subfolder.
    "OptiScaler",
    "fakenvapi.dll",
    "dlssg_to_fsr3_amd_is_better.dll",
];

/// Leftovers of a manual OptiScaler install in `install_dir` that would collide
/// with Proton's injected copy, as paths relative to `install_dir`.
///
/// `proxy` is the DLL name Proton injects OptiScaler under
/// (`PROTON_OPTISCALER_NAME`, default `dxgi.dll`): a game-folder file of that
/// name is the manual install's live entry point. It's only reported alongside
/// a marker — a lone `dxgi.dll` is as likely ReShade as OptiScaler.
///
/// Checks the root and, for Unreal games, each `<Project>/Binaries/Win64` —
/// where the game exe, and therefore any hand-installed proxy, actually sits.
pub fn manual_install_files(install_dir: &Path, proxy: &str) -> Vec<String> {
    let mut dirs = vec![PathBuf::new()];
    if let Ok(entries) = std::fs::read_dir(install_dir) {
        for entry in entries.flatten() {
            let ue = PathBuf::from(entry.file_name()).join("Binaries").join("Win64");
            if install_dir.join(&ue).is_dir() {
                dirs.push(ue);
            }
        }
    }

    let mut found = Vec::new();
    for rel in dirs {
        let Ok(entries) = std::fs::read_dir(install_dir.join(&rel)) else { continue };
        let names: Vec<String> =
            entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        let hit = |want: &str| names.iter().find(|n| n.eq_ignore_ascii_case(want));
        let markers: Vec<&String> = MANUAL_INSTALL_MARKERS.iter().filter_map(|m| hit(m)).collect();
        if markers.is_empty() {
            continue;
        }
        let path = |name: &String| rel.join(name).display().to_string();
        found.extend(hit(proxy).map(path));
        found.extend(markers.into_iter().map(path));
    }
    found
}

/// What's known about the latest upstream release on a channel — enough for
/// the "current vs latest" comparison and the confirm dialog's source/version
/// line.
#[derive(Clone, Debug, Serialize)]
pub struct OptiscalerRelease {
    pub channel: Channel,
    pub repo: String,
    pub tag: String,
    pub html_url: String,
    pub asset_name: String,
    #[serde(skip)]
    asset_url: String,
}

/// Query the newest release on `channel` and locate its `.7z` asset. Any
/// network / rate-limit / parse failure returns an error the caller surfaces
/// directly — unlike `update::check_blocking`, there's no banner to keep
/// quiet for.
pub fn check_latest(channel: Channel) -> Result<OptiscalerRelease, String> {
    let repo = channel.repo();
    match channel {
        Channel::Stable => {
            let url = format!("https://api.github.com/repos/{repo}/releases/latest");
            let v: serde_json::Value =
                serde_json::from_str(&fetch_text(&url, USER_AGENT)?).map_err(|e| e.to_string())?;
            parse_release(&v, channel)
        }
        // Every nightly is marked prerelease, so `/releases/latest` 404s there.
        // The list endpoint is newest-first; take the first published one that
        // actually carries an archive (a build still uploading has none yet).
        Channel::Nightly => {
            let url = format!("https://api.github.com/repos/{repo}/releases?per_page=5");
            let v: serde_json::Value =
                serde_json::from_str(&fetch_text(&url, USER_AGENT)?).map_err(|e| e.to_string())?;
            v.as_array()
                .into_iter()
                .flatten()
                .filter(|r| !r.get("draft").and_then(|x| x.as_bool()).unwrap_or(false))
                .find_map(|r| parse_release(r, channel).ok())
                .ok_or_else(|| format!("no recent {repo} release has a .7z asset"))
        }
    }
}

/// One GitHub release object → [`OptiscalerRelease`], or why it can't be used.
fn parse_release(v: &serde_json::Value, channel: Channel) -> Result<OptiscalerRelease, String> {
    let tag = v.get("tag_name").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let html_url = v.get("html_url").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let assets = v.get("assets").and_then(|x| x.as_array()).cloned().unwrap_or_default();

    // Versioned filename (e.g. "Optiscaler_0.9.4-final.20260718._MM.7z"), so
    // this matches by extension rather than a fixed name like `update.rs`'s
    // `asset_url` helper does for protongen's own release asset.
    let asset = assets
        .iter()
        .find(|a| a.get("name").and_then(|x| x.as_str()).is_some_and(|n| n.ends_with(".7z")))
        .ok_or_else(|| format!("OptiScaler release {tag} has no .7z asset"))?;
    let asset_name = asset.get("name").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let asset_url = asset
        .get("browser_download_url")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    if asset_url.is_empty() {
        return Err(format!("OptiScaler release {tag}'s asset has no download URL"));
    }

    Ok(OptiscalerRelease {
        channel,
        repo: channel.repo().to_string(),
        tag,
        html_url,
        asset_name,
        asset_url,
    })
}

/// What `fetch_and_extract` did, for the confirmation toast.
#[derive(Clone, Debug, Serialize)]
pub struct OptiscalerExtractResult {
    pub tag: String,
    pub files_written: usize,
    /// True when an existing `OptiScaler.ini` was left untouched.
    pub ini_preserved: bool,
    /// The name the new `OptiScaler.dll` was written as — the live proxy's
    /// (e.g. `dxgi.dll`) when the install is renamed.
    pub dll_name: String,
}

/// Download the latest release on `channel` and extract it into
/// `install_dir`, skipping `OptiScaler.ini` if one is already there and
/// writing `OptiScaler.dll` over the install's live proxy (see
/// [`entry_point`]) rather than beside it. Re-checks
/// the latest release itself rather than trusting a caller-supplied
/// [`OptiscalerRelease`], so the version reported back always matches what was
/// actually written.
pub fn fetch_and_extract(
    install_dir: &Path,
    channel: Channel,
) -> Result<OptiscalerExtractResult, String> {
    // Before the download: a stacked install is refused without fetching.
    let dll_name = entry_point(install_dir)?;
    let release = check_latest(channel)?;

    let archive_bytes = fetch_bytes_with(&release.asset_url, USER_AGENT, DOWNLOAD_TIMEOUT)?;
    if archive_bytes.is_empty() {
        return Err("downloaded OptiScaler release was empty".to_string());
    }

    let work = fresh_work_dir()?;
    let archive_path = work.join("OptiScaler.7z");
    let staged = work.join("extracted");
    let outcome = (|| {
        std::fs::write(&archive_path, &archive_bytes)
            .map_err(|e| format!("couldn't write {}: {e}", archive_path.display()))?;
        sevenz_rust2::decompress_file(&archive_path, &staged)
            .map_err(|e| format!("couldn't extract the OptiScaler archive: {e}"))?;
        copy_extracted(&staged, install_dir, &release.tag, dll_name.as_deref())
    })();
    // Best-effort cleanup either way — a leftover staging dir isn't worth
    // failing the whole operation over.
    let _ = std::fs::remove_dir_all(&work);
    outcome
}

/// A staging directory nobody else is using. It used to be one fixed name per
/// process, so two fetches at once (two games) extracted into the same place
/// and one's cleanup could delete the other's files mid-copy — and a fixed
/// name in a shared `/tmp` can be pre-created by someone else. `create_dir`
/// (not `_all`) fails if anything already sits at the name, symlinks included.
fn fresh_work_dir() -> Result<PathBuf, String> {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    for _ in 0..100 {
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir()
            .join(format!("protongen-optiscaler-{}-{nanos:x}-{n}", std::process::id()));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("couldn't create {}: {e}", dir.display())),
        }
    }
    Err("couldn't create a staging directory for OptiScaler".to_string())
}

/// Copy every file under `staged` into `install_dir`, preserving subfolders
/// (the archive ships `D3D12_Optiscaler/D3D12Core.dll`), skipping
/// [`INI_FILE`] when the destination already has one. The root
/// `OptiScaler.dll` is written as `dll_name` when given — the live proxy it
/// replaces.
///
/// Paths resolve against what's already there case-insensitively, as Wine
/// does: releases have shipped both `D3D12_OptiScaler` and
/// `D3D12_Optiscaler`, and on a case-sensitive filesystem a verbatim copy
/// leaves the old one beside the new one.
fn copy_extracted(
    staged: &Path,
    install_dir: &Path,
    tag: &str,
    dll_name: Option<&str>,
) -> Result<OptiscalerExtractResult, String> {
    let mut files_written = 0usize;
    let mut ini_preserved = false;

    for entry in walk_files(staged)? {
        let rel = entry
            .strip_prefix(staged)
            .map_err(|e| format!("internal path error: {e}"))?;
        let at_root = |name: &str| rel.to_str().is_some_and(|r| r.eq_ignore_ascii_case(name));

        if at_root(INI_FILE) && resolve_existing(install_dir, Path::new(INI_FILE)).exists() {
            ini_preserved = true;
            continue;
        }
        let dest = match dll_name.filter(|_| at_root(DLL_FILE)) {
            Some(name) => install_dir.join(name),
            None => resolve_existing(install_dir, rel),
        };
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("couldn't create {}: {e}", parent.display()))?;
        }
        std::fs::copy(&entry, &dest)
            .map_err(|e| format!("couldn't write {}: {e}", dest.display()))?;
        files_written += 1;
    }

    Ok(OptiscalerExtractResult {
        tag: tag.to_string(),
        files_written,
        ini_preserved,
        dll_name: dll_name.unwrap_or(DLL_FILE).to_string(),
    })
}

/// `root.join(rel)`, with each component swapped for an existing entry that
/// differs only in case. Components with no such entry are kept as given.
fn resolve_existing(root: &Path, rel: &Path) -> PathBuf {
    let mut out = root.to_path_buf();
    for part in rel.components() {
        let want = part.as_os_str();
        let existing = if out.join(want).exists() {
            None
        } else {
            let want = want.to_string_lossy();
            dir_names(&out).into_iter().find(|n| n.eq_ignore_ascii_case(&want))
        };
        match existing {
            Some(name) => out.push(name),
            None => out.push(want),
        }
    }
    out
}

/// Every regular file under `root`, recursively. No symlink handling —
/// nothing in the OptiScaler archive uses them.
fn walk_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| format!("couldn't read {}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                out.push(path);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_reports_found_only_when_a_marker_file_exists() {
        let dir = std::env::temp_dir().join("protongen-test-optiscaler-detect");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("OptiScaler.ini"), b"[Upscalers]\n").unwrap();

        let status = detect(Some(&dir));
        assert!(status.found);
        assert_eq!(status.install_dir.as_deref(), Some(dir.display().to_string().as_str()));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detect_reports_not_found_for_an_empty_or_unresolved_dir() {
        let dir = std::env::temp_dir().join("protongen-test-optiscaler-empty");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!detect(Some(&dir)).found);
        assert!(!detect(None).found);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn manual_install_files_lists_a_hand_install_with_its_proxy() {
        let dir = std::env::temp_dir().join("protongen-test-optiscaler-manual");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("OptiScaler")).unwrap();
        for f in ["dxgi.dll", "optiscaler.log", "fakenvapi.dll", "Game.exe"] {
            std::fs::write(dir.join(f), b"").unwrap();
        }

        assert_eq!(
            manual_install_files(&dir, "dxgi.dll"),
            vec!["dxgi.dll", "optiscaler.log", "OptiScaler", "fakenvapi.dll"]
        );
        // The proxy name follows PROTON_OPTISCALER_NAME.
        assert!(!manual_install_files(&dir, "winmm.dll").contains(&"dxgi.dll".to_string()));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn manual_install_files_ignores_a_lone_proxy_and_finds_unreal_layouts() {
        let dir = std::env::temp_dir().join("protongen-test-optiscaler-manual-ue");
        let _ = std::fs::remove_dir_all(&dir);
        let bin = dir.join("Project").join("Binaries").join("Win64");
        std::fs::create_dir_all(&bin).unwrap();
        // A root dxgi.dll with no OptiScaler beside it — ReShade, say.
        std::fs::write(dir.join("dxgi.dll"), b"").unwrap();
        assert!(manual_install_files(&dir, "dxgi.dll").is_empty());

        std::fs::write(bin.join("OptiScaler.ini"), b"").unwrap();
        assert_eq!(
            manual_install_files(&dir, "dxgi.dll"),
            vec!["Project/Binaries/Win64/OptiScaler.ini"]
        );
        assert!(manual_install_files(&dir.join("missing"), "dxgi.dll").is_empty());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parse_release_picks_the_7z_asset_and_tags_the_channel() {
        let v = serde_json::json!({
            "tag_name": "nightly-20260929",
            "html_url": "https://github.com/optiscaler/OptiScaler-nightly/releases/tag/nightly-20260929",
            "assets": [
                { "name": "notes.txt", "browser_download_url": "https://example/notes.txt" },
                { "name": "OptiScaler_v10.0.0-pre1_20260929.7z", "browser_download_url": "https://example/a.7z" }
            ]
        });
        let r = parse_release(&v, Channel::Nightly).unwrap();
        assert_eq!(r.tag, "nightly-20260929");
        assert_eq!(r.asset_name, "OptiScaler_v10.0.0-pre1_20260929.7z");
        assert_eq!(r.asset_url, "https://example/a.7z");
        assert_eq!(r.repo, "optiscaler/OptiScaler-nightly");
        assert_eq!(r.channel, Channel::Nightly);
    }

    #[test]
    fn parse_release_rejects_a_release_without_an_archive() {
        // The main repo's `nightly` tag is a changelog with no assets.
        let v = serde_json::json!({ "tag_name": "nightly", "html_url": "", "assets": [] });
        assert!(parse_release(&v, Channel::Stable).is_err());
    }

    #[test]
    fn channel_deserializes_from_lowercase() {
        let c: Channel = serde_json::from_str("\"nightly\"").unwrap();
        assert_eq!(c, Channel::Nightly);
        let c: Channel = serde_json::from_str("\"stable\"").unwrap();
        assert_eq!(c, Channel::Stable);
    }

    #[test]
    fn copy_extracted_skips_an_existing_ini_but_writes_everything_else() {
        let staged = std::env::temp_dir().join("protongen-test-optiscaler-staged");
        let dest = std::env::temp_dir().join("protongen-test-optiscaler-dest");
        std::fs::create_dir_all(staged.join("D3D12_Optiscaler")).unwrap();
        std::fs::create_dir_all(&dest).unwrap();

        std::fs::write(staged.join("OptiScaler.ini"), b"fresh from the release").unwrap();
        std::fs::write(staged.join("OptiScaler.dll"), b"dll bytes").unwrap();
        std::fs::write(staged.join("D3D12_Optiscaler").join("D3D12Core.dll"), b"nested dll").unwrap();
        std::fs::write(dest.join("OptiScaler.ini"), b"my tuned config").unwrap();

        let result = copy_extracted(&staged, &dest, "v0.9.4", None).unwrap();

        assert!(result.ini_preserved);
        assert_eq!(result.files_written, 2); // dll + nested dll, not the ini
        assert_eq!(
            std::fs::read_to_string(dest.join("OptiScaler.ini")).unwrap(),
            "my tuned config"
        );
        assert!(dest.join("D3D12_Optiscaler").join("D3D12Core.dll").exists());

        std::fs::remove_dir_all(&staged).unwrap();
        std::fs::remove_dir_all(&dest).unwrap();
    }

    #[test]
    fn copy_extracted_writes_the_ini_when_the_destination_has_none() {
        let staged = std::env::temp_dir().join("protongen-test-optiscaler-staged2");
        let dest = std::env::temp_dir().join("protongen-test-optiscaler-dest2");
        std::fs::create_dir_all(&staged).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(staged.join("OptiScaler.ini"), b"fresh from the release").unwrap();

        let result = copy_extracted(&staged, &dest, "v0.9.4", None).unwrap();

        assert!(!result.ini_preserved);
        assert_eq!(result.files_written, 1);
        assert_eq!(
            std::fs::read_to_string(dest.join("OptiScaler.ini")).unwrap(),
            "fresh from the release"
        );

        std::fs::remove_dir_all(&staged).unwrap();
        std::fs::remove_dir_all(&dest).unwrap();
    }

    /// What a proxy-renamed OptiScaler build looks like to [`is_optiscaler_dll`]:
    /// a binary carrying its own ini name among its strings.
    const OPTI_DLL: &[u8] = b"MZ\0\0...OptiScaler.ini\0...";

    #[test]
    fn detect_finds_the_renamed_proxy_and_a_stray_dll_beside_it() {
        let dir = std::env::temp_dir().join("protongen-test-optiscaler-proxy");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("OptiScaler.ini"), b"").unwrap();
        std::fs::write(dir.join("DXGI.dll"), OPTI_DLL).unwrap();
        // The game's own dbghelp.dll: right name, not OptiScaler.
        std::fs::write(dir.join("dbghelp.dll"), b"MZ\0\0Microsoft debug help").unwrap();

        let status = detect(Some(&dir));
        assert!(status.found);
        assert_eq!(status.proxies, vec!["DXGI.dll"]); // on-disk spelling
        assert!(!status.stray_dll);
        assert_eq!(entry_point(&dir).unwrap().as_deref(), Some("DXGI.dll"));

        // CONTROL Resonant: a pre-fix fetch left OptiScaler.dll beside the proxy.
        std::fs::write(dir.join("OptiScaler.dll"), OPTI_DLL).unwrap();
        assert!(detect(Some(&dir)).stray_dll);
        assert_eq!(entry_point(&dir).unwrap().as_deref(), Some("DXGI.dll"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detect_ignores_a_proxy_name_that_isnt_optiscaler() {
        let dir = std::env::temp_dir().join("protongen-test-optiscaler-reshade");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("dxgi.dll"), b"MZ\0\0ReShade").unwrap();
        assert!(!detect(Some(&dir)).found);

        // ReShade as dxgi.dll beside OptiScaler as winmm.dll — and the
        // UTF-16LE spelling some builds carry counts too.
        let wide: Vec<u8> = "OptiScaler.log".encode_utf16().flat_map(u16::to_le_bytes).collect();
        std::fs::write(dir.join("winmm.dll"), wide).unwrap();
        let status = detect(Some(&dir));
        assert!(status.found);
        assert_eq!(status.proxies, vec!["winmm.dll"]);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn entry_point_refuses_two_optiscaler_proxies() {
        let dir = std::env::temp_dir().join("protongen-test-optiscaler-two-proxies");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("dxgi.dll"), OPTI_DLL).unwrap();
        std::fs::write(dir.join("version.dll"), OPTI_DLL).unwrap();

        assert_eq!(detect(Some(&dir)).proxies, vec!["dxgi.dll", "version.dll"]);
        let err = entry_point(&dir).unwrap_err();
        assert!(err.contains("dxgi.dll, version.dll"), "{err}");
        // Not renamed at all → the archive's own name.
        std::fs::remove_file(dir.join("version.dll")).unwrap();
        std::fs::rename(dir.join("dxgi.dll"), dir.join("OptiScaler.dll")).unwrap();
        assert_eq!(entry_point(&dir).unwrap(), None);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn copy_extracted_writes_the_dll_over_the_live_proxy() {
        let staged = std::env::temp_dir().join("protongen-test-optiscaler-staged3");
        let dest = std::env::temp_dir().join("protongen-test-optiscaler-dest3");
        let _ = std::fs::remove_dir_all(&staged);
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&staged).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(staged.join("OptiScaler.dll"), b"new build").unwrap();
        std::fs::write(staged.join("libxess.dll"), b"new xess").unwrap();
        std::fs::write(dest.join("dxgi.dll"), b"old build").unwrap();

        let result = copy_extracted(&staged, &dest, "v0.9.5", Some("dxgi.dll")).unwrap();

        assert_eq!(result.dll_name, "dxgi.dll");
        assert_eq!(result.files_written, 2);
        assert_eq!(std::fs::read(dest.join("dxgi.dll")).unwrap(), b"new build");
        assert!(!dest.join("OptiScaler.dll").exists(), "the new build must not land beside the proxy");
        assert!(dest.join("libxess.dll").exists());

        std::fs::remove_dir_all(&staged).unwrap();
        std::fs::remove_dir_all(&dest).unwrap();
    }

    #[test]
    fn copy_extracted_reuses_existing_paths_that_differ_only_in_case() {
        let staged = std::env::temp_dir().join("protongen-test-optiscaler-staged4");
        let dest = std::env::temp_dir().join("protongen-test-optiscaler-dest4");
        let _ = std::fs::remove_dir_all(&staged);
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(staged.join("D3D12_Optiscaler")).unwrap();
        std::fs::create_dir_all(dest.join("D3D12_OptiScaler")).unwrap();
        std::fs::write(staged.join("D3D12_Optiscaler").join("D3D12Core.dll"), b"new").unwrap();
        std::fs::write(staged.join("OptiScaler.ini"), b"fresh from the release").unwrap();
        std::fs::write(staged.join("OptiScaler.dll"), b"new build").unwrap();
        std::fs::write(dest.join("D3D12_OptiScaler").join("D3D12Core.dll"), b"old").unwrap();
        std::fs::write(dest.join("optiscaler.ini"), b"my tuned config").unwrap();
        std::fs::write(dest.join("optiscaler.dll"), b"old build").unwrap();

        let result = copy_extracted(&staged, &dest, "v0.9.5", None).unwrap();

        assert!(result.ini_preserved);
        assert_eq!(result.dll_name, "OptiScaler.dll");
        let mut names = dir_names(&dest);
        names.sort();
        assert_eq!(names, vec!["D3D12_OptiScaler", "optiscaler.dll", "optiscaler.ini"]);
        assert_eq!(std::fs::read(dest.join("D3D12_OptiScaler").join("D3D12Core.dll")).unwrap(), b"new");
        assert_eq!(std::fs::read(dest.join("optiscaler.dll")).unwrap(), b"new build");
        assert_eq!(std::fs::read(dest.join("optiscaler.ini")).unwrap(), b"my tuned config");

        std::fs::remove_dir_all(&staged).unwrap();
        std::fs::remove_dir_all(&dest).unwrap();
    }
}
