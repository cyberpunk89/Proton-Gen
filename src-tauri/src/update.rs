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

use serde::{Deserialize, Serialize};

const REPO: &str = "cyberpunk89/Proton-Gen";
const USER_AGENT: &str = "protongen-updater";
/// The release asset name for the raw binary and its checksum file.
const BIN_ASSET: &str = "protongen";
const SHA_ASSET: &str = "protongen.sha256";

/// "Update available" banner data, and the download inputs for `run_update`.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    let body = fetch_text(&url)?;

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
    if info.download_url.is_empty() {
        return Err("release has no protongen binary asset".to_string());
    }
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = current_exe
        .parent()
        .ok_or_else(|| "cannot resolve install directory".to_string())?;

    let bin = fetch_bytes(&info.download_url)?;
    if bin.is_empty() {
        return Err("downloaded binary was empty".to_string());
    }

    // Verify against the published checksum when present ("<hex>  protongen").
    if !info.sha256_url.is_empty() {
        let sums = fetch_text(&info.sha256_url)?;
        let expected = sums.split_whitespace().next().unwrap_or_default().to_lowercase();
        if !expected.is_empty() {
            let got = sha256_hex(&bin);
            if got != expected {
                return Err(format!("checksum mismatch (expected {expected}, got {got})"));
            }
        }
    }

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

fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let mut req = ehttp::Request::get(url);
    // GitHub's API rejects requests without a User-Agent.
    req.headers.insert("User-Agent", USER_AGENT);
    req.headers.insert("Accept", "application/vnd.github+json");
    let resp = ehttp::fetch_blocking(&req)?;
    if !resp.ok {
        return Err(format!("HTTP {} {}", resp.status, resp.status_text));
    }
    Ok(resp.bytes)
}

fn fetch_text(url: &str) -> Result<String, String> {
    String::from_utf8(fetch_bytes(url)?).map_err(|e| e.to_string())
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
