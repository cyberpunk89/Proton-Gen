//! Read the user's *current* Steam configuration (read-only): the launch
//! options already set per game (`localconfig.vdf`) and the Proton tool mapped
//! per game (`config.vdf`, via steamlocate).

use std::collections::HashMap;

use keyvalues_parser::{Obj, Value};
use steamlocate::SteamDir;

/// Case-insensitive lookup of a child key's first value.
fn child<'a>(obj: &'a Obj<'a>, key: &str) -> Option<&'a Value<'a>> {
    obj.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, vs)| vs.first())
}

/// Parse one `localconfig.vdf` into `appid -> LaunchOptions`.
fn parse_localconfig(text: &str) -> HashMap<u32, String> {
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
        if let Some(opts) = vals
            .first()
            .and_then(|v| v.get_obj())
            .and_then(|o| child(o, "LaunchOptions"))
            .and_then(|v| v.get_str())
        {
            if !opts.is_empty() {
                out.insert(id, opts.to_string());
            }
        }
    }
    out
}

/// Current per-game launch options across all Steam users on this install.
pub fn current_launch_options(dir: &SteamDir) -> HashMap<u32, String> {
    let mut out = HashMap::new();
    let userdata = dir.path().join("userdata");
    let Ok(entries) = std::fs::read_dir(&userdata) else {
        return out;
    };
    for entry in entries.flatten() {
        let cfg = entry.path().join("config/localconfig.vdf");
        if let Ok(text) = std::fs::read_to_string(&cfg) {
            out.extend(parse_localconfig(&text));
        }
    }
    out
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

    #[test]
    fn extracts_launch_options() {
        let vdf = r#"
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
                    }
                    "275850"
                    {
                        "LaunchOptions"  ""
                    }
                }
            }
        }
    }
}
"#;
        let map = parse_localconfig(vdf);
        assert_eq!(
            map.get(&553850).map(String::as_str),
            Some("PROTON_USE_NTSYNC=1 mangohud %command%")
        );
        // Empty options are skipped.
        assert!(!map.contains_key(&275850));
    }
}
