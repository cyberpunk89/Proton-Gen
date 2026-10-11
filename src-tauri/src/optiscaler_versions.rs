//! "Is this OptiScaler current?" — the build of every OptiScaler the library
//! has, against the newest upstream one. Report-only: upgrading a manual
//! install stays the confirm-gated fetch in `optiscaler_upgrade`, and Proton's
//! own copy is Proton's to update.
//!
//! Two kinds of install, versioned two ways:
//!
//! - **Manual**, in the game folder: the DLL's own startup banner
//!   (`OptiScaler v0.9.4-final (99cc3772)`), which every build carries as a
//!   plain ASCII string. Read as bytes — nothing is loaded or run.
//! - **Proton**, from `PROTON_USE_OPTISCALER`: proton-cachyos's tracking file
//!   `upscaler_files` (in the compat dir: beside `pfx/` for Steam, inside the
//!   prefix for umu), which records the version it copied into
//!   `system32/umu/`. Proton re-syncs that copy against the proton-upscalers
//!   manifest at every launch, so it is judged against the manifest's newest
//!   build — an older copy is "refreshed next launch", not something to fix.
//!   Proton never deletes the copy when injection is turned off, so a hit only
//!   means the game was launched with it at some point.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::optiscaler_upgrade::{self, Channel, OptiscalerRelease};

const USER_AGENT: &str = "protongen-optiscaler-versions";

/// Where proton-cachyos (`protonfixes/upscalers.py`) reads which OptiScaler
/// builds it can inject.
const PROTON_MANIFEST_URL: &str = "https://loathingkernel.github.io/proton-upscalers/manifest.json";

/// Upstream answers are reused for this long (and across launches, via
/// `disk_cache`): releases land daily at most, and GitHub's unauthenticated
/// API allows 60 calls an hour.
const DISK_TTL: Duration = Duration::from_secs(6 * 3600);

/// What an OptiScaler DLL says about itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct OptiscalerBuild {
    /// As the banner spells it, without the `v`: `0.9.4-final`, `10.0.0-pre1`.
    pub version: String,
    pub commit: Option<String>,
    /// `YYYYMMDD`, for builds that stamp their build time (dev / nightly).
    pub built: Option<String>,
}

/// `OptiScaler v10.0.0-dev (44cfee4) (20260926_085051)` → its parts. `None`
/// unless a version starting with a digit follows the `v`.
pub fn parse_banner(s: &str) -> Option<OptiscalerBuild> {
    let rest = s.trim().strip_prefix("OptiScaler v")?;
    let (version, tail) = rest.split_once(' ').unwrap_or((rest, ""));
    if !version.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let mut commit = None;
    let mut built = None;
    for group in tail.split(['(', ')']).map(str::trim).filter(|g| !g.is_empty()) {
        if group.len() >= 8 && group[..8].bytes().all(|b| b.is_ascii_digit()) {
            built.get_or_insert_with(|| group[..8].to_string());
        } else if group.bytes().all(|b| b.is_ascii_hexdigit()) {
            commit.get_or_insert_with(|| group.to_string());
        }
    }
    Some(OptiscalerBuild { version: version.to_string(), commit, built })
}

/// The banner inside a DLL's bytes. The format string next to it
/// (`OptiScaler v{}`) is skipped by requiring a digit after the `v`.
fn banner_in(bytes: &[u8]) -> Option<OptiscalerBuild> {
    const NEEDLE: &[u8] = b"OptiScaler v";
    let mut from = 0;
    while let Some(i) = bytes[from..].windows(NEEDLE.len()).position(|w| w == NEEDLE) {
        let start = from + i;
        let end = bytes[start..]
            .iter()
            .take(160)
            .position(|&b| b == 0 || !(0x20..0x7f).contains(&b))
            .map_or(bytes.len().min(start + 160), |n| start + n);
        if let Some(build) = std::str::from_utf8(&bytes[start..end]).ok().and_then(parse_banner) {
            return Some(build);
        }
        from = start + NEEDLE.len();
    }
    None
}

/// The build of the OptiScaler DLL at `path`, remembered by size + mtime: the
/// files run to 30 MB, and the overview re-asks after every rescan.
pub fn dll_build(path: &Path) -> Option<OptiscalerBuild> {
    type Memo = HashMap<PathBuf, (u64, Option<SystemTime>, Option<OptiscalerBuild>)>;
    static MEMO: Mutex<Option<Memo>> = Mutex::new(None);

    let meta = std::fs::metadata(path).ok()?;
    let stamp = (meta.len(), meta.modified().ok());
    if let Ok(memo) = MEMO.lock()
        && let Some((len, mtime, build)) = memo.as_ref().and_then(|m| m.get(path))
        && (*len, *mtime) == stamp
    {
        return build.clone();
    }
    let build = std::fs::read(path).ok().and_then(|b| banner_in(&b));
    if let Ok(mut memo) = MEMO.lock() {
        memo.get_or_insert_with(HashMap::new)
            .insert(path.to_path_buf(), (stamp.0, stamp.1, build.clone()));
    }
    build
}

/// A comparable form of an OptiScaler version: the numbers, a hotfix letter
/// (`0.9.2a`), then the stage — `dev` < `preN` < `rcN` < final/none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    nums: [u64; 3],
    letter: u8,
    stage: u8,
    stage_n: u64,
}

fn key(v: &str) -> Option<Key> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let num_end = v.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(v.len());
    let parts: Vec<u64> = v[..num_end]
        .split('.')
        .filter(|p| !p.is_empty())
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let mut nums = [0; 3];
    nums[..parts.len()].copy_from_slice(&parts);

    let mut rest = &v[num_end..];
    let mut letter = 0;
    if let Some(c) = rest.chars().next().filter(char::is_ascii_lowercase)
        && !rest[1..].starts_with(|c: char| c.is_ascii_alphanumeric())
    {
        letter = c as u8 - b'a' + 1;
        rest = &rest[1..];
    }
    let tag: String = rest
        .strip_prefix('-')
        .unwrap_or("")
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    let word: String = tag.chars().take_while(char::is_ascii_alphabetic).collect();
    let stage_n = tag[word.len()..].parse().unwrap_or(0);
    let stage = match word.as_str() {
        "dev" | "alpha" => 0,
        "pre" | "beta" => 1,
        "rc" => 2,
        _ => 3,
    };
    Some(Key { nums, letter, stage, stage_n })
}

/// The version a release asset carries: `Optiscaler_0.9.4-final.20260718._MM.7z`
/// → `0.9.4-final`, `OptiScaler_v10.0.0-pre1_20261011.7z` → `10.0.0-pre1`.
fn asset_version(asset: &str) -> Option<String> {
    let lower = asset.to_ascii_lowercase();
    let rest = &asset[lower.find("optiscaler_")? + "optiscaler_".len()..];
    let rest = rest.trim_start_matches(['v', 'V']);
    let mut v = &rest[..rest.find('_').unwrap_or(rest.len())];
    // Cut at the `.YYYYMMDD` date stamp the stable names carry.
    if let Some(i) = v.match_indices('.').map(|(i, _)| i).find(|&i| {
        v[i + 1..].len() >= 8 && v[i + 1..i + 9].bytes().all(|b| b.is_ascii_digit())
    }) {
        v = &v[..i];
    }
    let v = v.trim_end_matches(".7z");
    key(v).map(|_| v.to_string())
}

/// The first run of exactly eight digits — a `YYYYMMDD` stamp.
fn eight_digit_run(s: &str) -> Option<String> {
    s.split(|c: char| !c.is_ascii_digit()).find(|r| r.len() == 8).map(str::to_string)
}

/// The newest upstream build on one channel, as the overview reports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "OptiscalerUpstream", export_to = "../../src/lib/generated/"))]
pub struct UpstreamBuild {
    pub channel: Channel,
    pub version: String,
    pub tag: String,
    pub html_url: String,
    /// `YYYYMMDD` from the asset name (release date for stable, build date for
    /// nightly).
    pub built: Option<String>,
}

impl UpstreamBuild {
    fn from_release(r: &OptiscalerRelease) -> Self {
        UpstreamBuild {
            channel: r.channel,
            version: asset_version(&r.asset_name)
                .unwrap_or_else(|| r.tag.trim_start_matches('v').to_string()),
            tag: r.tag.clone(),
            html_url: r.html_url.clone(),
            built: eight_digit_run(&r.asset_name),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "OptiscalerInstallKind", export_to = "../../src/lib/generated/"))]
pub enum InstallKind {
    /// Dropped into the game folder by hand (or by this app's fetch).
    Manual,
    /// Copied into the prefix by proton-cachyos for `PROTON_USE_OPTISCALER`.
    Proton,
}

/// How an install compares with upstream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "OptiscalerFreshness", export_to = "../../src/lib/generated/"))]
pub enum Freshness {
    /// The newest build of its line (stable, or nightly for a nightly build).
    Current,
    /// A manual install with a newer upstream build — the fetch upgrades it.
    Outdated,
    /// Newer than anything upstream reported (a dev build, say).
    Ahead,
    /// A Proton copy older than proton-upscalers' newest: Proton swaps it on
    /// the game's next launch.
    ProtonRefresh,
    /// A Proton copy that is proton-upscalers' newest, while OptiScaler itself
    /// has a newer release that proton-upscalers hasn't packaged yet.
    ProtonWaiting,
    /// Unreadable version, or nothing upstream to compare against.
    Unknown,
}

/// One OptiScaler in the library.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct OptiscalerInstall {
    pub app_id: u32,
    pub kind: InstallKind,
    /// The folder holding it: the game's install dir, or the prefix.
    pub dir: String,
    /// The file the build was read from (`dxgi.dll`, `OptiScaler.dll`, …).
    pub dll: String,
    pub build: Option<OptiscalerBuild>,
    pub freshness: Freshness,
    /// What it would move to, when that differs from what it is.
    pub latest: Option<String>,
}

/// Every OptiScaler in the library plus the upstream builds they were judged
/// against.
#[derive(Clone, Debug, Default, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct OptiscalerOverview {
    pub installs: Vec<OptiscalerInstall>,
    pub stable: Option<UpstreamBuild>,
    /// Only looked up when some install is a nightly-line build.
    pub nightly: Option<UpstreamBuild>,
    /// proton-upscalers' newest OptiScaler, when a Proton copy needed it.
    pub proton_latest: Option<String>,
    /// Lookups that failed, for a quiet footnote rather than an error.
    pub errors: Vec<String>,
}

/// Where one game's OptiScaler could be.
pub struct Target {
    pub app_id: u32,
    pub install_dir: Option<PathBuf>,
    pub prefix: Option<PathBuf>,
}

/// A manual install's live DLL (its proxy, or `OptiScaler.dll` when it isn't
/// renamed) — the same file the fetch would write over. `Some(None)` is an
/// install `detect` sees (an `OptiScaler.ini`, say) with no DLL to read.
fn manual_install(dir: &Path) -> Option<Option<String>> {
    let status = optiscaler_upgrade::detect(Some(dir));
    if !status.found {
        return None;
    }
    Some(status.proxies.first().cloned().or_else(|| {
        std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .find(|n| n.eq_ignore_ascii_case("OptiScaler.dll"))
    }))
}

/// The OptiScaler version proton-cachyos recorded for `prefix`, from its
/// `upscaler_files` (umu: inside the prefix; Steam: beside `pfx/`).
pub fn proton_version(prefix: &Path) -> Option<String> {
    [Some(prefix), prefix.parent()].into_iter().flatten().find_map(|dir| {
        let text = std::fs::read_to_string(dir.join("upscaler_files")).ok()?;
        let v: serde_json::Value = serde_json::from_str(&text).ok()?;
        v.get("opti_files")?
            .as_object()?
            .values()
            .find_map(|f| f.get("version")?.as_str().map(str::to_string))
    })
}

/// What was found on disk, before any network.
struct Found {
    app_id: u32,
    kind: InstallKind,
    dir: PathBuf,
    dll: String,
    build: Option<OptiscalerBuild>,
}

fn scan(targets: &[Target]) -> Vec<Found> {
    let mut out = Vec::new();
    for t in targets {
        if let Some(dir) = t.install_dir.as_deref()
            && let Some(dll) = manual_install(dir)
        {
            let build = dll.as_ref().and_then(|d| dll_build(&dir.join(d)));
            out.push(Found {
                app_id: t.app_id,
                kind: InstallKind::Manual,
                dir: dir.to_path_buf(),
                dll: dll.unwrap_or_default(),
                build,
            });
        }
        if let Some(prefix) = t.prefix.as_deref()
            && let Some(version) = proton_version(prefix)
        {
            out.push(Found {
                app_id: t.app_id,
                kind: InstallKind::Proton,
                dir: prefix.to_path_buf(),
                dll: "system32/umu/OptiScaler.dll".to_string(),
                build: Some(OptiscalerBuild { version, commit: None, built: None }),
            });
        }
    }
    out
}

/// Whether `build` belongs to the nightly line: newer than the latest stable,
/// or a pre-release stage.
fn is_nightly_line(build: &OptiscalerBuild, stable: Option<&UpstreamBuild>) -> bool {
    let Some(k) = key(&build.version) else { return false };
    k.stage < 3 || stable.and_then(|s| key(&s.version)).is_some_and(|s| k > s)
}

/// Judge one install. Pure: everything upstream is passed in.
fn freshness(
    kind: InstallKind,
    build: Option<&OptiscalerBuild>,
    stable: Option<&UpstreamBuild>,
    nightly: Option<&UpstreamBuild>,
    proton_latest: Option<&str>,
) -> (Freshness, Option<String>) {
    let Some(build) = build else { return (Freshness::Unknown, None) };
    let Some(have) = key(&build.version) else { return (Freshness::Unknown, None) };

    if kind == InstallKind::Proton {
        let Some(pkg) = proton_latest.and_then(|p| key(p).map(|k| (k, p))) else {
            return (Freshness::Unknown, None);
        };
        if have < pkg.0 {
            return (Freshness::ProtonRefresh, Some(pkg.1.to_string()));
        }
        return match stable.and_then(|s| key(&s.version).map(|k| (k, s))) {
            // Compared on the numbers only: the manifest says `0.9.4` where the
            // release says `0.9.4-final`.
            Some((k, s)) if (k.nums, k.letter) > (have.nums, have.letter) => {
                (Freshness::ProtonWaiting, Some(s.version.clone()))
            }
            _ => (Freshness::Current, None),
        };
    }

    if is_nightly_line(build, stable) {
        let Some(n) = nightly else { return (Freshness::Unknown, None) };
        // Every nightly shares a version for weeks; the build date tells them apart.
        let ord = match (&build.built, &n.built) {
            (Some(a), Some(b)) => a.cmp(b),
            _ => match key(&n.version) {
                Some(nk) => have.cmp(&nk),
                None => return (Freshness::Unknown, None),
            },
        };
        return match ord {
            std::cmp::Ordering::Less => {
                let latest = match &n.built {
                    Some(b) => format!("{} ({b})", n.version),
                    None => n.version.clone(),
                };
                (Freshness::Outdated, Some(latest))
            }
            std::cmp::Ordering::Equal => (Freshness::Current, None),
            std::cmp::Ordering::Greater => (Freshness::Ahead, None),
        };
    }

    let Some(s) = stable else { return (Freshness::Unknown, None) };
    match key(&s.version).map(|k| have.cmp(&k)) {
        Some(std::cmp::Ordering::Less) => (Freshness::Outdated, Some(s.version.clone())),
        Some(std::cmp::Ordering::Equal) => (Freshness::Current, None),
        Some(std::cmp::Ordering::Greater) => (Freshness::Ahead, None),
        None => (Freshness::Unknown, None),
    }
}

/// Assemble the overview from what's on disk and what upstream said. Pure.
fn judge(
    found: Vec<Found>,
    stable: Option<UpstreamBuild>,
    nightly: Option<UpstreamBuild>,
    proton_latest: Option<String>,
    errors: Vec<String>,
) -> OptiscalerOverview {
    let installs = found
        .into_iter()
        .map(|f| {
            let (freshness, latest) = freshness(
                f.kind,
                f.build.as_ref(),
                stable.as_ref(),
                nightly.as_ref(),
                proton_latest.as_deref(),
            );
            OptiscalerInstall {
                app_id: f.app_id,
                kind: f.kind,
                dir: f.dir.display().to_string(),
                dll: f.dll,
                build: f.build,
                freshness,
                latest,
            }
        })
        .collect();
    OptiscalerOverview { installs, stable, nightly, proton_latest, errors }
}

/// The newest release on `channel`, from the disk cache when fresh.
fn cached_latest(channel: Channel) -> Result<UpstreamBuild, String> {
    let name = match channel {
        Channel::Stable => "optiscaler-latest-stable.json",
        Channel::Nightly => "optiscaler-latest-nightly.json",
    };
    if let Some(b) = crate::disk_cache::get::<UpstreamBuild>(name, DISK_TTL) {
        return Ok(b);
    }
    let b = UpstreamBuild::from_release(&optiscaler_upgrade::check_latest(channel)?);
    crate::disk_cache::put(name, &b);
    Ok(b)
}

/// proton-upscalers' newest non-dev OptiScaler — the build proton-cachyos
/// injects by default.
fn cached_proton_latest() -> Result<String, String> {
    const NAME: &str = "optiscaler-proton-upscalers.json";
    if let Some(v) = crate::disk_cache::get::<String>(NAME, DISK_TTL) {
        return Ok(v);
    }
    let text = crate::update::fetch_text(PROTON_MANIFEST_URL, USER_AGENT)?;
    let v = proton_manifest_latest(&text).ok_or("proton-upscalers lists no OptiScaler build")?;
    crate::disk_cache::put(NAME, &v);
    Ok(v)
}

/// The last non-dev `optiscaler` entry — proton-cachyos' own `default` pick.
fn proton_manifest_latest(manifest: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(manifest).ok()?;
    v.get("optiscaler")?
        .as_array()?
        .iter()
        .filter(|e| !e.get("is_dev_file").and_then(|d| d.as_bool()).unwrap_or(false))
        .filter_map(|e| e.get("version")?.as_str())
        .last()
        .map(str::to_string)
}

/// Scan `targets` and compare against upstream. Network only for what was
/// found: no OptiScaler anywhere means no request at all.
pub fn overview(targets: &[Target]) -> OptiscalerOverview {
    let found = scan(targets);
    let mut errors = Vec::new();
    fn note<T>(errors: &mut Vec<String>, what: &str, r: Result<T, String>) -> Option<T> {
        r.map_err(|e| {
            eprintln!("OptiScaler {what} check failed: {e}");
            errors.push(format!("{what}: {e}"));
        })
        .ok()
    }
    let stable = (!found.is_empty())
        .then(|| note(&mut errors, "stable release", cached_latest(Channel::Stable)))
        .flatten();
    let wants_nightly = found.iter().any(|f| {
        f.kind == InstallKind::Manual && f.build.as_ref().is_some_and(|b| is_nightly_line(b, stable.as_ref()))
    });
    let nightly = wants_nightly
        .then(|| note(&mut errors, "nightly build", cached_latest(Channel::Nightly)))
        .flatten();
    let proton_latest = found
        .iter()
        .any(|f| f.kind == InstallKind::Proton)
        .then(|| note(&mut errors, "proton-upscalers manifest", cached_proton_latest()))
        .flatten();
    judge(found, stable, nightly, proton_latest, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn up(channel: Channel, version: &str, built: Option<&str>) -> UpstreamBuild {
        UpstreamBuild {
            channel,
            version: version.into(),
            tag: format!("v{version}"),
            html_url: String::new(),
            built: built.map(str::to_string),
        }
    }

    fn build(version: &str, built: Option<&str>) -> OptiscalerBuild {
        OptiscalerBuild { version: version.into(), commit: None, built: built.map(str::to_string) }
    }

    #[test]
    fn parses_stable_and_dev_banners() {
        assert_eq!(
            parse_banner("OptiScaler v0.9.4-final (99cc3772)"),
            Some(OptiscalerBuild {
                version: "0.9.4-final".into(),
                commit: Some("99cc3772".into()),
                built: None
            })
        );
        assert_eq!(
            parse_banner("OptiScaler v10.0.0-dev (44cfee4) (20260926_085051)"),
            Some(OptiscalerBuild {
                version: "10.0.0-dev".into(),
                commit: Some("44cfee4".into()),
                built: Some("20260926".into())
            })
        );
        assert_eq!(parse_banner("OptiScaler v{}"), None);
        assert_eq!(parse_banner("OptiScaler is freely downloadable"), None);
    }

    #[test]
    fn finds_the_banner_past_the_format_string() {
        let bytes = b"MZ\0\0OptiScaler v{} loaded\0\0OptiScaler v0.9.4-final (7534ad0)\0\0- %s";
        assert_eq!(banner_in(bytes).map(|b| b.version), Some("0.9.4-final".into()));
        assert_eq!(banner_in(b"MZ\0\0ReShade 6.3"), None);
    }

    #[test]
    fn orders_versions_by_number_hotfix_then_stage() {
        let k = |v| key(v).unwrap();
        assert!(k("0.9.4-final") > k("0.9.3-final"));
        assert!(k("0.9.2a-final") > k("0.9.2-final"));
        assert!(k("10.0.0-pre1") > k("0.9.4-final"));
        assert!(k("10.0.0-pre1") > k("10.0.0-dev"));
        assert!(k("10.0.0-pre2") > k("10.0.0-pre1"));
        assert!(k("10.0.0") > k("10.0.0-rc1"));
        assert_eq!(k("0.9.4"), k("v0.9.4-final"));
        assert_eq!(key("nightly"), None);
    }

    #[test]
    fn reads_versions_out_of_asset_names() {
        let v = |a| asset_version(a);
        assert_eq!(v("Optiscaler_0.9.4-final.20260718._MM.7z").as_deref(), Some("0.9.4-final"));
        assert_eq!(v("Optiscaler_0.9.3-final.20260618.7z").as_deref(), Some("0.9.3-final"));
        assert_eq!(v("Optiscaler_0.9.2a-final.20260517._Reup.7z").as_deref(), Some("0.9.2a-final"));
        assert_eq!(v("OptiScaler_v10.0.0-pre1_20261011.7z").as_deref(), Some("10.0.0-pre1"));
        assert_eq!(v("notes.txt"), None);
        assert_eq!(eight_digit_run("OptiScaler_v10.0.0-pre1_20261011.7z").as_deref(), Some("20261011"));
    }

    #[test]
    fn judges_manual_installs_against_stable() {
        let stable = up(Channel::Stable, "0.9.4-final", Some("20260718"));
        let f = |v: &str| freshness(InstallKind::Manual, Some(&build(v, None)), Some(&stable), None, None);
        assert_eq!(f("0.9.3-final"), (Freshness::Outdated, Some("0.9.4-final".into())));
        assert_eq!(f("0.9.4-final"), (Freshness::Current, None));
        assert_eq!(freshness(InstallKind::Manual, None, Some(&stable), None, None).0, Freshness::Unknown);
        assert_eq!(freshness(InstallKind::Manual, Some(&build("0.9.3", None)), None, None, None).0, Freshness::Unknown);
    }

    #[test]
    fn judges_nightly_builds_by_build_date() {
        let stable = up(Channel::Stable, "0.9.4-final", None);
        let nightly = up(Channel::Nightly, "10.0.0-pre1", Some("20261011"));
        let f = |b: OptiscalerBuild| {
            freshness(InstallKind::Manual, Some(&b), Some(&stable), Some(&nightly), None)
        };
        assert_eq!(
            f(build("10.0.0-pre1", Some("20261002"))),
            (Freshness::Outdated, Some("10.0.0-pre1 (20261011)".into()))
        );
        assert_eq!(f(build("10.0.0-pre1", Some("20261011"))).0, Freshness::Current);
        assert_eq!(f(build("10.0.0-dev", Some("20261012"))).0, Freshness::Ahead);
        // No nightly info: can't tell.
        assert_eq!(
            freshness(InstallKind::Manual, Some(&build("10.0.0-pre1", None)), Some(&stable), None, None).0,
            Freshness::Unknown
        );
    }

    #[test]
    fn judges_proton_copies_against_proton_upscalers() {
        let stable = up(Channel::Stable, "0.9.4-final", None);
        let f = |have: &str, pkg: &str| {
            freshness(InstallKind::Proton, Some(&build(have, None)), Some(&stable), None, Some(pkg))
        };
        assert_eq!(f("0.9.3", "0.9.4"), (Freshness::ProtonRefresh, Some("0.9.4".into())));
        assert_eq!(f("0.9.4", "0.9.4"), (Freshness::Current, None));
        let newer = up(Channel::Stable, "0.9.5-final", None);
        assert_eq!(
            freshness(InstallKind::Proton, Some(&build("0.9.4", None)), Some(&newer), None, Some("0.9.4")),
            (Freshness::ProtonWaiting, Some("0.9.5-final".into()))
        );
    }

    #[test]
    fn picks_the_newest_non_dev_build_from_the_proton_manifest() {
        let m = r#"{"optiscaler": [
            {"version": "0.9.3", "is_dev_file": false},
            {"version": "0.9.4", "is_dev_file": false},
            {"version": "0.9.5-dev", "is_dev_file": true}
        ], "dlss": []}"#;
        assert_eq!(proton_manifest_latest(m).as_deref(), Some("0.9.4"));
        assert_eq!(proton_manifest_latest("{}"), None);
    }

    #[test]
    fn scans_a_manual_install_and_a_proton_prefix() {
        let root = std::env::temp_dir().join("protongen-test-optiscaler-versions");
        let _ = std::fs::remove_dir_all(&root);
        let game = root.join("game");
        let compat = root.join("compatdata");
        let pfx = compat.join("pfx");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::create_dir_all(&pfx).unwrap();
        std::fs::write(game.join("OptiScaler.ini"), b"").unwrap();
        std::fs::write(game.join("dxgi.dll"), b"MZ\0OptiScaler.ini\0OptiScaler v0.9.3-final (abc1234)\0").unwrap();
        // Steam layout: the tracking file sits beside pfx/.
        std::fs::write(
            compat.join("upscaler_files"),
            r#"{"opti_files": {"drive_c/windows/system32/umu/OptiScaler.dll": {"version": "0.9.4", "md5_hash": "x"}}}"#,
        )
        .unwrap();

        let found = scan(&[Target { app_id: 7, install_dir: Some(game.clone()), prefix: Some(pfx.clone()) }]);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].kind, InstallKind::Manual);
        assert_eq!(found[0].dll, "dxgi.dll");
        assert_eq!(found[0].build.as_ref().map(|b| b.version.as_str()), Some("0.9.3-final"));
        assert_eq!(found[1].kind, InstallKind::Proton);
        assert_eq!(found[1].build.as_ref().map(|b| b.version.as_str()), Some("0.9.4"));

        let o = judge(found, Some(up(Channel::Stable, "0.9.4-final", None)), None, Some("0.9.4".into()), vec![]);
        assert_eq!(o.installs[0].freshness, Freshness::Outdated);
        assert_eq!(o.installs[1].freshness, Freshness::Current);

        // Nothing there → nothing reported.
        assert!(scan(&[Target { app_id: 8, install_dir: Some(root.join("none")), prefix: None }]).is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
