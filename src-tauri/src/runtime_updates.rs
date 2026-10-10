//! "A newer build of a Proton you use is out" — a read-only notice, never an
//! installer (installing Proton stays a non-goal, see `design.md` §1.4).
//!
//! Only families the user already has installed are checked, so someone who
//! never installed GE-Proton is never nagged to. Valve's own Proton builds are
//! left out on purpose: Steam updates those by itself.
//!
//! Split like `update.rs`: the comparison is pure and unit-tested, and the
//! network half only fetches each family's latest GitHub release.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::update::fetch_text;

const USER_AGENT: &str = "protongen-runtime-updates";

/// The Proton builds this notice knows how to version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub enum Family {
    GeProton,
    ProtonCachyos,
}

impl Family {
    const ALL: [Family; 2] = [Family::GeProton, Family::ProtonCachyos];

    fn repo(self) -> &'static str {
        match self {
            Family::GeProton => "GloriousEggroll/proton-ge-custom",
            Family::ProtonCachyos => "CachyOS/proton-cachyos",
        }
    }

    /// A comparable version out of a runtime name or release tag, plus the
    /// part worth showing. GE: `GE-Proton11-5` → (11, 5). proton-cachyos: its
    /// 8-digit build date, the same key `runtime::installed_cachyos_build` and
    /// the stale-catalog banner already use.
    fn version(self, s: &str) -> Option<(u64, u64, String)> {
        match self {
            Family::GeProton => {
                let i = s.find("GE-Proton")?;
                let rest = &s[i + "GE-Proton".len()..];
                let (major, rest) = split_number(rest)?;
                let (minor, _) = split_number(rest.strip_prefix('-')?)?;
                Some((major, minor, format!("GE-Proton{major}-{minor}")))
            }
            Family::ProtonCachyos => {
                if !s.to_lowercase().contains("cachyos") {
                    return None;
                }
                let date = eight_digit_run(s)?;
                Some((date.parse().ok()?, 0, date))
            }
        }
    }
}

/// Leading decimal number of `s`, and what follows it.
fn split_number(s: &str) -> Option<(u64, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    Some((s[..end].parse().ok()?, &s[end..]))
}

/// The first run of exactly eight digits (a `YYYYMMDD` build date).
fn eight_digit_run(s: &str) -> Option<String> {
    s.split(|c: char| !c.is_ascii_digit())
        .find(|run| run.len() == 8)
        .map(str::to_string)
}

/// One installed runtime, as far as this module cares.
pub struct Installed<'a> {
    pub name: &'a str,
    /// `runtime::RuntimeKind::label()` — `system` means a distro package.
    pub kind: &'a str,
}

/// An upstream release, reduced to what the comparison needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Upstream {
    pub family: Family,
    pub tag: String,
    pub html_url: String,
}

/// A family where upstream is ahead of the newest installed build.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct RuntimeUpdate {
    pub family: Family,
    pub installed: String,
    pub latest: String,
    /// The release tag, used as the dismissal key.
    pub tag: String,
    pub html_url: String,
    /// Kind of the newest installed build, so the frontend can say how to
    /// update it: `system` (a pacman package) vs a hand-installed folder.
    pub installed_kind: String,
}

/// Compare the newest installed build of each family against its upstream
/// release. Pure — the whole decision, so it's what the tests cover.
pub fn compare(installed: &[Installed], upstream: &[Upstream]) -> Vec<RuntimeUpdate> {
    let mut out = Vec::new();
    for up in upstream {
        let Some((la, lb, latest)) = up.family.version(&up.tag) else { continue };
        let newest = installed
            .iter()
            .filter_map(|r| up.family.version(r.name).map(|(a, b, shown)| (a, b, shown, r.kind)))
            .max_by_key(|(a, b, ..)| (*a, *b));
        let Some((ia, ib, shown, kind)) = newest else { continue };
        if (la, lb) > (ia, ib) {
            out.push(RuntimeUpdate {
                family: up.family,
                installed: shown,
                latest,
                tag: up.tag.clone(),
                html_url: up.html_url.clone(),
                installed_kind: kind.to_string(),
            });
        }
    }
    out
}

/// Upstream releases already fetched this session. A rescan (say, after the
/// user installed the new build) re-runs [`compare`] against these instead of
/// asking GitHub again; a failed fetch isn't cached, so it's retried.
static UPSTREAM: Mutex<Vec<Upstream>> = Mutex::new(Vec::new());

/// How long a family's latest release is trusted on disk across launches.
/// Both families ship at most a few builds a week.
const DISK_TTL: std::time::Duration = std::time::Duration::from_secs(12 * 3600);

fn disk_name(f: Family) -> &'static str {
    match f {
        Family::GeProton => "runtime-latest-ge-proton.json",
        Family::ProtonCachyos => "runtime-latest-proton-cachyos.json",
    }
}

/// Fetch the latest release of every family the user has installed, then
/// [`compare`]. A family whose fetch fails is skipped rather than failing the
/// rest; this backs a background notice with nothing useful to say on error.
/// Families missing from the session cache come from disk if fresh, else from
/// GitHub — and those network fetches run in parallel.
pub fn check_blocking(installed: &[Installed]) -> Vec<RuntimeUpdate> {
    let mut upstream = UPSTREAM.lock().map(|u| u.clone()).unwrap_or_default();
    let wanted: Vec<Family> = Family::ALL
        .into_iter()
        .filter(|f| installed.iter().any(|r| f.version(r.name).is_some()))
        .filter(|f| !upstream.iter().any(|u| u.family == *f))
        .collect();
    let fetched: Vec<Upstream> = std::thread::scope(|s| {
        let handles: Vec<_> = wanted
            .iter()
            .map(|&f| {
                s.spawn(move || {
                    if let Some(u) = crate::disk_cache::get::<Upstream>(disk_name(f), DISK_TTL) {
                        return Some(u);
                    }
                    match fetch_latest(f) {
                        Ok(u) => {
                            crate::disk_cache::put(disk_name(f), &u);
                            Some(u)
                        }
                        Err(e) => {
                            eprintln!("runtime update check for {} failed: {e}", f.repo());
                            None
                        }
                    }
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok().flatten()).collect()
    });
    if let Ok(mut cache) = UPSTREAM.lock() {
        cache.extend(fetched.iter().cloned());
    }
    upstream.extend(fetched);
    compare(installed, &upstream)
}

fn fetch_latest(family: Family) -> Result<Upstream, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", family.repo());
    let v: serde_json::Value =
        serde_json::from_str(&fetch_text(&url, USER_AGENT)?).map_err(|e| e.to_string())?;
    let tag = v.get("tag_name").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let html_url = v.get("html_url").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    if tag.is_empty() {
        return Err("release has no tag".to_string());
    }
    // Only ever link into the family's own repo, whatever the API returned.
    let expected = format!("https://github.com/{}/", family.repo());
    let html_url = if html_url.starts_with(&expected) {
        html_url
    } else {
        format!("{expected}releases")
    };
    Ok(Upstream { family, tag, html_url })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn up(family: Family, tag: &str) -> Upstream {
        Upstream { family, tag: tag.into(), html_url: format!("https://x/{tag}") }
    }

    #[test]
    fn parses_ge_and_cachyos_versions() {
        let ge = Family::GeProton;
        assert_eq!(ge.version("GE-Proton11-5-x86_64").map(|v| (v.0, v.1)), Some((11, 5)));
        assert_eq!(ge.version("GE-Proton10-34").map(|v| v.2), Some("GE-Proton10-34".into()));
        assert_eq!(ge.version("GE-Proton"), None); // umu's codename, no version
        assert_eq!(ge.version("proton-cachyos-11.0-20260703"), None);

        let c = Family::ProtonCachyos;
        assert_eq!(
            c.version("proton-cachyos-11.0-20260703 (steam linux runtime)").map(|v| v.2),
            Some("20260703".into())
        );
        assert_eq!(c.version("cachyos-11.0-20260915-slr").map(|v| v.0), Some(20260915));
        assert_eq!(c.version("GE-Proton11-5"), None);
    }

    #[test]
    fn reports_a_newer_upstream_against_the_newest_installed_build() {
        let installed = [
            Installed { name: "GE-Proton10-34", kind: "user" },
            Installed { name: "GE-Proton11-5-x86_64", kind: "user" },
        ];
        let got = compare(&installed, &[up(Family::GeProton, "GE-Proton11-7")]);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].installed, "GE-Proton11-5");
        assert_eq!(got[0].latest, "GE-Proton11-7");
        assert_eq!(got[0].installed_kind, "user");

        // 11-10 > 11-9 numerically, not as strings.
        let installed = [Installed { name: "GE-Proton11-9", kind: "user" }];
        assert_eq!(compare(&installed, &[up(Family::GeProton, "GE-Proton11-10")]).len(), 1);
    }

    #[test]
    fn stays_quiet_when_current_or_not_installed() {
        let installed = [Installed {
            name: "proton-cachyos-11.0-20260703 (steam linux runtime)",
            kind: "system",
        }];
        let upstream = [
            up(Family::ProtonCachyos, "cachyos-11.0-20260703-slr"),
            // Not installed at all: never suggested.
            up(Family::GeProton, "GE-Proton11-7"),
        ];
        assert!(compare(&installed, &upstream).is_empty());

        let got = compare(&installed, &[up(Family::ProtonCachyos, "cachyos-11.0-20260915-slr")]);
        assert_eq!(got[0].installed, "20260703");
        assert_eq!(got[0].latest, "20260915");
        assert_eq!(got[0].installed_kind, "system");
    }
}
