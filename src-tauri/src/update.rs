//! In-app self-update against GitHub Releases.
//!
//! The app is installed as a bare binary at `~/.local/bin/protongen` (via
//! `install.sh`, `tauri build --no-bundle`), so the official Tauri updater — which
//! replaces a bundled AppImage/deb — is a poor fit. Instead we check the GitHub
//! Releases API and, on request, download the new `protongen` binary asset, verify
//! its SHA-256, and atomically swap it in place (replacing a running binary on
//! Linux is safe: the open inode persists, the new version takes effect on
//! restart). No code signing: integrity comes from HTTPS + the published checksum.
//!
//! HTTP mirrors the `protondb`/`art` modules: `ehttp::fetch_blocking`, wrapped by
//! the caller in `spawn_blocking` so it never stalls the UI.

use serde::Serialize;

const REPO: &str = "cyberpunk89/Proton-Gen";
const USER_AGENT: &str = "protongen-updater";
/// The release asset name for the raw binary and its checksum file.
const BIN_ASSET: &str = "protongen";
const SHA_ASSET: &str = "protongen.sha256";

/// "Update available" banner data, and the download inputs for `run_update`.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct UpdateInfo {
    pub available: bool,
    pub current: String,
    pub latest: String,
    pub notes: String,
    pub html_url: String,
    pub download_url: String,
    pub sha256_url: String,
}

/// Query the latest GitHub release and compare it to the running version.
/// Any network / rate-limit / parse failure returns an error the caller can
/// swallow — a failed check must never block launch or surface a false banner.
pub fn check_blocking() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body = fetch_text(&url, USER_AGENT)?;

    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let tag = v.get("tag_name").and_then(|x| x.as_str()).unwrap_or_default();
    let latest = tag.trim_start_matches('v').to_string();
    let notes = v.get("body").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let html_url = v.get("html_url").and_then(|x| x.as_str()).unwrap_or_default().to_string();

    let assets = v.get("assets").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let download_url = asset_url(&assets, BIN_ASSET);
    let sha256_url = asset_url(&assets, SHA_ASSET);

    let newer = match (semver::Version::parse(&latest), semver::Version::parse(&current)) {
        (Ok(l), Ok(c)) => l > c,
        _ => false,
    };

    Ok(UpdateInfo {
        available: newer && !download_url.is_empty(),
        current,
        latest,
        notes,
        html_url,
        download_url,
        sha256_url,
    })
}

/// Download the new binary, verify its checksum, and atomically replace the
/// currently-running executable. Caller restarts the app on success.
pub fn download_and_swap(info: &UpdateInfo) -> Result<(), String> {
    if !info.available {
        return Err(format!("v{} is not newer than the running v{}", info.latest, info.current));
    }
    if info.download_url.is_empty() {
        return Err("release has no protongen binary asset".to_string());
    }
    // The checksum is the only integrity check there is (no code signing), so a
    // release without one is refused rather than installed on trust.
    if info.sha256_url.is_empty() {
        return Err(format!(
            "release v{} has no {SHA_ASSET} checksum, so it can't be verified — \
             install it manually from {} or re-run install.sh",
            info.latest, info.html_url
        ));
    }
    for url in [&info.download_url, &info.sha256_url] {
        if !is_release_asset_url(url) {
            return Err(format!("refusing to download {url}: not a {REPO} release asset"));
        }
    }
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = current_exe
        .parent()
        .ok_or_else(|| "cannot resolve install directory".to_string())?;

    let bin = fetch_bytes_with(&info.download_url, USER_AGENT, DOWNLOAD_TIMEOUT)?;
    if bin.is_empty() {
        return Err("downloaded binary was empty".to_string());
    }
    verify_checksum(&bin, &fetch_text(&info.sha256_url, USER_AGENT)?)?;

    // Write into the install dir first so the final rename is atomic (same fs).
    let tmp = dir.join(".protongen.update.tmp");
    std::fs::write(&tmp, &bin).map_err(|e| format!("write temp file: {e}"))?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &current_exe).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!(
            "could not replace {} ({e}). Re-run install.sh to update manually.",
            current_exe.display()
        )
    })?;
    Ok(())
}

/// Only this repo's own release downloads are ever fetched and installed.
fn is_release_asset_url(url: &str) -> bool {
    url.strip_prefix(&format!("https://github.com/{REPO}/releases/download/"))
        .is_some_and(|rest| !rest.is_empty() && !rest.contains(".."))
}

fn asset_url(assets: &[serde_json::Value], name: &str) -> String {
    assets
        .iter()
        .find_map(|a| {
            if a.get("name").and_then(|x| x.as_str()) == Some(name) {
                a.get("browser_download_url").and_then(|x| x.as_str()).map(String::from)
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// GitHub rejects requests without a `User-Agent`; shared by every module that
/// hits its API (`update`, `optiscaler_upgrade`).
pub(crate) fn fetch_bytes(url: &str, user_agent: &str) -> Result<Vec<u8>, String> {
    fetch_bytes_with(url, user_agent, ehttp::Request::DEFAULT_TIMEOUT)
}

/// How long a release-asset download may take to arrive. ehttp's timeout
/// covers receiving the whole body, and its 30 s default fails a binary or a
/// tens-of-MB archive partway on a slow link.
pub(crate) const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// [`fetch_bytes`] with an explicit body timeout.
pub(crate) fn fetch_bytes_with(
    url: &str,
    user_agent: &str,
    timeout: std::time::Duration,
) -> Result<Vec<u8>, String> {
    let mut req = ehttp::Request::get(url).with_timeout(Some(timeout));
    req.headers.insert("User-Agent", user_agent);
    req.headers.insert("Accept", "application/vnd.github+json");
    let resp = ehttp::fetch_blocking(&req)?;
    if !resp.ok {
        return Err(format!("HTTP {} {}", resp.status, resp.status_text));
    }
    Ok(resp.bytes)
}

pub(crate) fn fetch_text(url: &str, user_agent: &str) -> Result<String, String> {
    String::from_utf8(fetch_bytes(url, user_agent)?).map_err(|e| e.to_string())
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Check `bin` against a `sha256sum`-style file (`"<hex>  protongen"`). An
/// empty or malformed file is an error, not a pass.
fn verify_checksum(bin: &[u8], sums: &str) -> Result<(), String> {
    let expected = sums.split_whitespace().next().unwrap_or_default().to_lowercase();
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{SHA_ASSET} is malformed — refusing to install an unverified binary"));
    }
    let got = sha256_hex(bin);
    if got != expected {
        return Err(format!("checksum mismatch (expected {expected}, got {got})"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_this_repos_release_assets_are_accepted() {
        assert!(is_release_asset_url(
            "https://github.com/cyberpunk89/Proton-Gen/releases/download/v0.21.0/protongen"
        ));
        for bad in [
            "https://evil.example/protongen",
            "http://github.com/cyberpunk89/Proton-Gen/releases/download/v1/protongen",
            "https://github.com/someone/else/releases/download/v1/protongen",
            "https://github.com/cyberpunk89/Proton-Gen/releases/download/../../x",
            "https://github.com/cyberpunk89/Proton-Gen/releases/download/",
        ] {
            assert!(!is_release_asset_url(bad), "{bad}");
        }
    }

    // sha256("hello")
    const HELLO: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    fn info(sha256_url: &str) -> UpdateInfo {
        UpdateInfo {
            available: true,
            current: "0.1.0".into(),
            latest: "9.9.9".into(),
            notes: String::new(),
            html_url: "https://example.invalid/release".into(),
            download_url: "https://example.invalid/protongen".into(),
            sha256_url: sha256_url.into(),
        }
    }

    #[test]
    fn a_release_without_a_checksum_is_refused_before_downloading() {
        let err = download_and_swap(&info("")).unwrap_err();
        assert!(err.contains("checksum"), "{err}");
        assert!(err.contains("https://example.invalid/release"), "names the manual path: {err}");
    }

    #[test]
    fn verify_checksum_accepts_a_match_in_either_case() {
        assert!(verify_checksum(b"hello", &format!("{HELLO}  protongen\n")).is_ok());
        assert!(verify_checksum(b"hello", &HELLO.to_uppercase()).is_ok());
    }

    #[test]
    fn verify_checksum_rejects_a_mismatch() {
        let err = verify_checksum(b"tampered", HELLO).unwrap_err();
        assert!(err.contains("mismatch"), "{err}");
    }

    #[test]
    fn verify_checksum_rejects_empty_or_malformed_files() {
        for bad in ["", "   \n", "not-a-hash  protongen", &HELLO[..63], &format!("{}zz", &HELLO[..62])] {
            assert!(verify_checksum(b"hello", bad).is_err(), "accepted {bad:?}");
        }
    }
}
