//! Read the user's *current* Steam configuration (read-only): the per-app user
//! settings recorded in `localconfig.vdf` (launch options, last-played,
//! playtime) and the Proton tool mapped per game (`config.vdf`, via steamlocate).

use std::collections::HashMap;

use keyvalues_parser::{Obj, Value};
use steamlocate::SteamDir;

/// Per-app settings recorded for one Steam user in `localconfig.vdf`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AppUserCfg {
    /// `LaunchOptions` — empty when the user never set any.
    pub launch_options: String,
    /// `LastPlayed`, unix seconds. `None` when the app was never launched.
    pub last_played: Option<u64>,
    /// `Playtime`, total minutes.
    pub playtime_minutes: Option<u32>,
}

impl AppUserCfg {
    fn is_empty(&self) -> bool {
        self.launch_options.is_empty()
            && self.last_played.is_none()
            && self.playtime_minutes.is_none()
    }

    /// Fold another Steam user's record for the same app into this one.
    ///
    /// `last_played` / `playtime_minutes` take the `max()`: "when was this game
    /// last played on this machine" is a property of the machine, not of
    /// whichever user directory happened to be read last.
    ///
    /// `launch_options` cannot be merged that way — two users can legitimately
    /// set different strings for the same app and there is no "greater" one. The
    /// winner here is simply the last non-empty value seen, i.e. **filesystem
    /// iteration order over `userdata/`, which is arbitrary and may differ
    /// between runs**. That was already true before this function existed, but
    /// it matters more now: the value feeds the in-sync / drifted verdict (#29),
    /// so on a multi-user install that verdict is only as stable as the readdir
    /// order. Picking the *right* user needs the logged-in SteamID, which this
    /// read-only scan deliberately does not resolve.
    fn merge(&mut self, other: AppUserCfg) {
        if !other.launch_options.is_empty() {
            self.launch_options = other.launch_options;
        }
        self.last_played = self.last_played.max(other.last_played);
        self.playtime_minutes = self.playtime_minutes.max(other.playtime_minutes);
    }
}

/// Case-insensitive lookup of a child key's first value.
fn child<'a>(obj: &'a Obj<'a>, key: &str) -> Option<&'a Value<'a>> {
    obj.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, vs)| vs.first())
}

/// Case-insensitive lookup of a child key parsed as an integer. Steam writes
/// these as quoted decimal strings.
fn child_num<T: std::str::FromStr>(obj: &Obj<'_>, key: &str) -> Option<T> {
    child(obj, key)?.get_str()?.trim().parse().ok()
}

/// Parse one `localconfig.vdf` into `appid -> AppUserCfg`.
fn parse_localconfig(text: &str) -> HashMap<u32, AppUserCfg> {
    let mut out = HashMap::new();
    let Ok(vdf) = keyvalues_parser::parse(text) else {
        return out;
    };
    // Root value is the contents of "UserLocalConfigStore".
    let Some(root) = vdf.value.get_obj() else {
        return out;
    };
    // Navigate Software → Valve → Steam → apps.
    let mut node = root;
    for key in ["Software", "Valve", "Steam", "apps"] {
        match child(node, key).and_then(|v| v.get_obj()) {
            Some(o) => node = o,
            None => return out,
        }
    }
    for (appid, vals) in node.iter() {
        let Ok(id) = appid.parse::<u32>() else { continue };
        let Some(app) = vals.first().and_then(|v| v.get_obj()) else {
            continue;
        };
        let cfg = AppUserCfg {
            launch_options: child(app, "LaunchOptions")
                .and_then(|v| v.get_str())
                .unwrap_or_default()
                .to_string(),
            last_played: child_num(app, "LastPlayed").filter(|&t| t > 0),
            playtime_minutes: child_num(app, "Playtime"),
        };
        // An app node with nothing we read tells us nothing; skip it so callers
        // can treat "present" as "Steam recorded something about this app".
        if !cfg.is_empty() {
            out.insert(id, cfg);
        }
    }
    out
}

/// Per-app user settings merged across every Steam user on this install.
/// See [`AppUserCfg::merge`] for how conflicting values are resolved.
pub fn current_app_cfgs(dir: &SteamDir) -> HashMap<u32, AppUserCfg> {
    let mut out: HashMap<u32, AppUserCfg> = HashMap::new();
    let userdata = dir.path().join("userdata");
    let Ok(entries) = std::fs::read_dir(&userdata) else {
        return out;
    };
    for entry in entries.flatten() {
        let cfg = entry.path().join("config/localconfig.vdf");
        if let Ok(text) = std::fs::read_to_string(&cfg) {
            for (id, app) in parse_localconfig(&text) {
                out.entry(id).or_default().merge(app);
            }
        }
    }
    out
}

/// Current per-game launch options (apps with none set are omitted).
pub fn launch_options(cfgs: &HashMap<u32, AppUserCfg>) -> HashMap<u32, String> {
    cfgs.iter()
        .filter(|(_, c)| !c.launch_options.is_empty())
        .map(|(id, c)| (*id, c.launch_options.clone()))
        .collect()
}

/// Current per-game compat tool (internal name) from `config.vdf`.
pub fn current_compat_tools(dir: &SteamDir) -> HashMap<u32, String> {
    dir.compat_tool_mapping()
        .map(|m| {
            m.into_iter()
                .filter_map(|(appid, tool)| tool.name.map(|n| (appid, n)))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VDF: &str = r#"
"UserLocalConfigStore"
{
    "Software"
    {
        "Valve"
        {
            "Steam"
            {
                "apps"
                {
                    "553850"
                    {
                        "LaunchOptions"  "PROTON_USE_NTSYNC=1 mangohud %command%"
                        "LastPlayed"     "1751000000"
                        "Playtime"       "4210"
                    }
                    "275850"
                    {
                        "LaunchOptions"  ""
                        "LastPlayed"     "1740000000"
                    }
                    "1245620"
                    {
                        "Playtime"       "12"
                    }
                    "999999"
                    {
                        "SomethingElse"  "1"
                    }
                }
            }
        }
    }
}
"#;

    #[test]
    fn extracts_launch_options() {
        let map = parse_localconfig(VDF);
        assert_eq!(
            map.get(&553850).map(|c| c.launch_options.as_str()),
            Some("PROTON_USE_NTSYNC=1 mangohud %command%")
        );
        // An app with no launch options still parses; the string is just empty.
        assert_eq!(map.get(&275850).map(|c| c.launch_options.as_str()), Some(""));
        // `launch_options()` is what drops the empty ones.
        let opts = launch_options(&map);
        assert!(opts.contains_key(&553850));
        assert!(!opts.contains_key(&275850));
    }

    #[test]
    fn extracts_last_played_and_playtime() {
        let map = parse_localconfig(VDF);
        let hd2 = map.get(&553850).expect("553850 present");
        assert_eq!(hd2.last_played, Some(1_751_000_000));
        assert_eq!(hd2.playtime_minutes, Some(4210));
        // LastPlayed without launch options is still surfaced.
        assert_eq!(map.get(&275850).unwrap().last_played, Some(1_740_000_000));
        // Playtime alone is enough to keep the entry.
        let er = map.get(&1245620).expect("1245620 present");
        assert_eq!(er.last_played, None);
        assert_eq!(er.playtime_minutes, Some(12));
        // An app node with nothing we read is dropped entirely.
        assert!(!map.contains_key(&999999));
    }

    #[test]
    fn merge_takes_max_last_played_across_users() {
        let mut a = AppUserCfg {
            launch_options: "mangohud %command%".to_string(),
            last_played: Some(100),
            playtime_minutes: Some(30),
        };
        // A second user who played it more recently but set no launch options
        // must not blank the ones we already have, and must win on time.
        a.merge(AppUserCfg {
            launch_options: String::new(),
            last_played: Some(500),
            playtime_minutes: Some(10),
        });
        assert_eq!(a.launch_options, "mangohud %command%");
        assert_eq!(a.last_played, Some(500));
        assert_eq!(a.playtime_minutes, Some(30));

        // An older record merged second still loses on time.
        a.merge(AppUserCfg {
            launch_options: String::new(),
            last_played: Some(1),
            playtime_minutes: None,
        });
        assert_eq!(a.last_played, Some(500));
    }

    #[test]
    fn zero_last_played_reads_as_never() {
        let vdf = r#"
"UserLocalConfigStore"
{
    "Software" { "Valve" { "Steam" { "apps" {
        "1" { "LastPlayed" "0" "Playtime" "0" }
    } } } }
}
"#;
        let map = parse_localconfig(vdf);
        // Playtime 0 is a real value; LastPlayed 0 is Steam's "never".
        assert_eq!(map.get(&1).unwrap().last_played, None);
        assert_eq!(map.get(&1).unwrap().playtime_minutes, Some(0));
    }
}
