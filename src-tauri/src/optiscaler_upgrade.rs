//! Fetch the latest OptiScaler release from GitHub and extract it into a
//! game's install directory — the manual "grab the newest OptiScaler build
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

use serde::Serialize;

use crate::update::{fetch_bytes, fetch_text};

const REPO: &str = "optiscaler/OptiScaler";
const USER_AGENT: &str = "protongen-optiscaler-upgrade";

/// Files that mark a game folder as already having OptiScaler installed —
/// either by hand or by CachyOS Proton's own `PROTON_USE_OPTISCALER`
/// auto-injection. Only a folder with one of these present is offered the
/// fetch action: the point is to refresh an existing install with a newer
/// upstream build, never to inject OptiScaler into a game that isn't using it.
const MARKER_FILES: &[&str] = &["OptiScaler.dll", "OptiScaler.ini"];

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
}

/// Detect an existing install. `install_dir` is `None` when nothing could be
/// resolved for this game (see `games::Game::install_dir`).
pub fn detect(install_dir: Option<&Path>) -> OptiscalerStatus {
    let found = install_dir.is_some_and(|dir| MARKER_FILES.iter().any(|f| dir.join(f).exists()));
    OptiscalerStatus { install_dir: install_dir.map(|p| p.display().to_string()), found }
}

/// What's known about the latest upstream release — enough for the "current
/// vs latest" comparison and the confirm dialog's source/version line.
#[derive(Clone, Debug, Serialize)]
pub struct OptiscalerRelease {
    pub tag: String,
    pub html_url: String,
    pub asset_name: String,
    #[serde(skip)]
    asset_url: String,
}

/// Query the latest GitHub release and locate its `.7z` asset. Any network /
/// rate-limit / parse failure returns an error the caller surfaces directly —
/// unlike `update::check_blocking`, there's no banner to keep quiet for.
pub fn check_latest() -> Result<OptiscalerRelease, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body = fetch_text(&url, USER_AGENT)?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;

    let tag = v.get("tag_name").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let html_url = v.get("html_url").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let assets = v.get("assets").and_then(|x| x.as_array()).cloned().unwrap_or_default();

    // Versioned filename (e.g. "Optiscaler_0.9.4-final.20260718._MM.7z"), so
    // this matches by extension rather than a fixed name like `update.rs`'s
    // `asset_url` helper does for protongen's own release asset.
    let asset = assets
        .iter()
        .find(|a| a.get("name").and_then(|x| x.as_str()).is_some_and(|n| n.ends_with(".7z")))
        .ok_or_else(|| "latest OptiScaler release has no .7z asset".to_string())?;
    let asset_name = asset.get("name").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let asset_url = asset
        .get("browser_download_url")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    if asset_url.is_empty() {
        return Err("latest OptiScaler release's asset has no download URL".to_string());
    }

    Ok(OptiscalerRelease { tag, html_url, asset_name, asset_url })
}

/// What `fetch_and_extract` did, for the confirmation toast.
#[derive(Clone, Debug, Serialize)]
pub struct OptiscalerExtractResult {
    pub tag: String,
    pub files_written: usize,
    /// True when an existing `OptiScaler.ini` was left untouched.
    pub ini_preserved: bool,
}

/// Download the latest release and extract it into `install_dir`, skipping
/// `OptiScaler.ini` if one is already there. Re-checks the latest release
/// itself rather than trusting a caller-supplied [`OptiscalerRelease`], so the
/// version reported back always matches what was actually written.
pub fn fetch_and_extract(install_dir: &Path) -> Result<OptiscalerExtractResult, String> {
    let release = check_latest()?;

    let archive_bytes = fetch_bytes(&release.asset_url, USER_AGENT)?;
    if archive_bytes.is_empty() {
        return Err("downloaded OptiScaler release was empty".to_string());
    }

    let work = std::env::temp_dir().join(format!("protongen-optiscaler-{}", std::process::id()));
    let archive_path = work.with_extension("7z");
    std::fs::create_dir_all(&work).map_err(|e| format!("couldn't create {}: {e}", work.display()))?;
    std::fs::write(&archive_path, &archive_bytes)
        .map_err(|e| format!("couldn't write {}: {e}", archive_path.display()))?;

    let extract_result = sevenz_rust2::decompress_file(&archive_path, &work)
        .map_err(|e| format!("couldn't extract the OptiScaler archive: {e}"));

    // Best-effort cleanup either way — a leftover temp archive/staging dir
    // isn't worth failing the whole operation over.
    let _ = std::fs::remove_file(&archive_path);
    extract_result.map_err(|e| {
        let _ = std::fs::remove_dir_all(&work);
        e
    })?;

    let outcome = copy_extracted(&work, install_dir, &release.tag);
    let _ = std::fs::remove_dir_all(&work);
    outcome
}

/// Copy every file under `staged` into `install_dir`, preserving subfolders
/// (the archive ships `D3D12_Optiscaler/D3D12Core.dll`), skipping
/// [`INI_FILE`] when the destination already has one.
fn copy_extracted(
    staged: &Path,
    install_dir: &Path,
    tag: &str,
) -> Result<OptiscalerExtractResult, String> {
    let mut files_written = 0usize;
    let mut ini_preserved = false;

    for entry in walk_files(staged)? {
        let rel = entry
            .strip_prefix(staged)
            .map_err(|e| format!("internal path error: {e}"))?;

        if rel == Path::new(INI_FILE) && install_dir.join(INI_FILE).exists() {
            ini_preserved = true;
            continue;
        }

        let dest = install_dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("couldn't create {}: {e}", parent.display()))?;
        }
        std::fs::copy(&entry, &dest)
            .map_err(|e| format!("couldn't write {}: {e}", dest.display()))?;
        files_written += 1;
    }

    Ok(OptiscalerExtractResult { tag: tag.to_string(), files_written, ini_preserved })
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
    fn copy_extracted_skips_an_existing_ini_but_writes_everything_else() {
        let staged = std::env::temp_dir().join("protongen-test-optiscaler-staged");
        let dest = std::env::temp_dir().join("protongen-test-optiscaler-dest");
        std::fs::create_dir_all(staged.join("D3D12_Optiscaler")).unwrap();
        std::fs::create_dir_all(&dest).unwrap();

        std::fs::write(staged.join("OptiScaler.ini"), b"fresh from the release").unwrap();
        std::fs::write(staged.join("OptiScaler.dll"), b"dll bytes").unwrap();
        std::fs::write(staged.join("D3D12_Optiscaler").join("D3D12Core.dll"), b"nested dll").unwrap();
        std::fs::write(dest.join("OptiScaler.ini"), b"my tuned config").unwrap();

        let result = copy_extracted(&staged, &dest, "v0.9.4").unwrap();

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

        let result = copy_extracted(&staged, &dest, "v0.9.4").unwrap();

        assert!(!result.ini_preserved);
        assert_eq!(result.files_written, 1);
        assert_eq!(
            std::fs::read_to_string(dest.join("OptiScaler.ini")).unwrap(),
            "fresh from the release"
        );

        std::fs::remove_dir_all(&staged).unwrap();
        std::fs::remove_dir_all(&dest).unwrap();
    }
}
