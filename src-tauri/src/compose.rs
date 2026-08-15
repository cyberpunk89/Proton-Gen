//! Config -> launch string. The whole assembly pipeline in one pure function:
//! `Config` -> `Options` -> `params::to_spec` -> `builder`.
//!
//! This sits *above* both `builder` and `params` on purpose: `params` already
//! depends on `builder`, so putting it in `builder.rs` would cycle. Keeping it
//! out of `ipc` lets non-command callers (batch status, tests, the CLI) run the
//! exact same pipeline the preview uses.

use crate::builder;
use crate::params::{self, Catalog, Options};
use crate::parser;
use crate::store::{self, Config};

/// Split the "custom env" field (`K=V K=V …`) into pairs. Uses the parser's
/// quote-aware splitter so `FOO="a b"` survives as a single pair.
pub fn parse_extra_env(s: &str) -> Vec<(String, String)> {
    parser::tokenize(s)
        .iter()
        .filter_map(|t| t.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect()
}

/// The exact inverse of [`parse_extra_env`]: render pairs back into the "custom
/// env" field's `K=V K=V …` form, re-quoting any value containing whitespace so
/// `("FOO", "a b")` round-trips instead of shearing into two tokens next time.
pub fn format_extra_env(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| {
            if v.contains(char::is_whitespace) {
                format!("{k}=\"{v}\"")
            } else {
                format!("{k}={v}")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Append `pairs` to an `extra_env` string, skipping any key it already assigns.
///
/// Dedup is **by key, and the incoming pair loses**. These pairs are the
/// leftovers from [`store::options_from_lists`] — keys a catalog refresh
/// forgot — so their values come from a catalog the app no longer has. What the
/// user typed into the visible custom-env field must outrank that.
///
/// Deliberately *not* `recipes::apply`'s rule, which dedups by whole `K=V` token
/// and lets the recipe win: applying a recipe is the user asking for that value,
/// recovering a stale key is not.
pub fn merge_into_extra_env(extra_env: &str, pairs: &[(String, String)]) -> String {
    let existing: Vec<String> = parse_extra_env(extra_env).into_iter().map(|(k, _)| k).collect();
    let fresh: Vec<(String, String)> = pairs
        .iter()
        .filter(|(k, _)| !existing.contains(k))
        .cloned()
        .collect();
    if fresh.is_empty() {
        return extra_env.to_string();
    }
    let rendered = format_extra_env(&fresh);
    if extra_env.trim().is_empty() {
        rendered
    } else {
        format!("{} {rendered}", extra_env.trim_end())
    }
}

/// Rebuild `Options` from a `Config`'s catalog env/wrapper lists, plus the env
/// pairs the catalog has no entry for. See [`store::options_from_lists`] — the
/// leftovers must be re-homed, not discarded (#62).
pub fn options_from_config(catalog: &Catalog, config: &Config) -> (Options, Vec<(String, String)>) {
    store::options_from_lists(catalog, &config.env, &config.wrappers)
}

/// Resolve a `Config` into the final environment variables and wrappers, the way
/// the preview does. Shared by [`assemble`] and `ipc::inject_heroic` so what is
/// written into a Heroic config is byte-identical to what the preview shows —
/// including the #62 re-homing of catalog-forgotten keys into the env.
///
/// This is the *mode-agnostic* core: it excludes the umu lead vars
/// (WINEPREFIX/GAMEID/PROTONPATH), which `assemble` adds only for umu mode and
/// which Heroic owns itself.
pub fn resolve_env_wrappers(
    catalog: &Catalog,
    config: &Config,
) -> (Vec<(String, String)>, Vec<builder::Wrapper>) {
    let (options, leftover) = options_from_config(catalog, config);
    let (mut env, wrappers) = params::to_spec(catalog, &options);
    // One merge, so the preview and `ipc::apply_recipe` cannot disagree about
    // what the custom-env field holds.
    env.extend(parse_extra_env(&merge_into_extra_env(
        &config.extra_env,
        &leftover,
    )));
    (env, wrappers)
}

/// Assemble the launch command for `config`. `proton_path` is the selected
/// runtime's install dir, used as `PROTONPATH` in umu mode and ignored otherwise.
pub fn assemble(
    catalog: &Catalog,
    config: &Config,
    proton_path: Option<&str>,
    bins: &builder::Bins,
) -> String {
    let (env, wrappers) = resolve_env_wrappers(catalog, config);

    if config.umu {
        let wineprefix = {
            let wp = config.umu_wineprefix.trim();
            if wp.is_empty() { None } else { Some(wp) }
        };
        builder::build_umu_command(
            &env,
            &wrappers,
            proton_path.unwrap_or(""),
            &config.umu_gameid,
            wineprefix,
            &config.umu_exe,
            &config.game_args,
            bins,
        )
    } else {
        builder::build_command(&env, &wrappers, &config.game_args, bins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assemble_reproduces_all_combined_fixture() {
        // Byte-identical to builder::tests::all_combined, but driven the way the
        // app drives it. PROTON_USE_NTSYNC sits in `env`, not `extra_env`: the
        // catalog dropped that key (Proton 11 kept only PROTON_NO_NTSYNC), so it
        // comes back from `options_from_lists` as a leftover and merges into the
        // custom-env string — which is why the output still matches byte for
        // byte. Before #62 the fixture had to hand-stage the key in `extra_env`,
        // because assembling it from `env` silently lost it.
        let cat = Catalog::bundled();
        let config = Config {
            env: vec![("PROTON_USE_NTSYNC".to_string(), "1".to_string())],
            wrappers: vec![
                ("gamemoderun".to_string(), String::new()),
                ("mangohud".to_string(), String::new()),
                ("gamescope".to_string(), "-f".to_string()),
            ],
            game_args: "--skip-launcher".to_string(),
            ..Config::default()
        };
        assert_eq!(
            assemble(&cat, &config, None, &builder::Bins::default()),
            "PROTON_USE_NTSYNC=1 gamescope -f -- gamemoderun mangohud %command% --skip-launcher"
        );
    }

    #[test]
    fn catalog_env_is_enabled_by_key() {
        let cat = Catalog::bundled();
        let config = Config {
            env: vec![("PROTON_ENABLE_WAYLAND".to_string(), "1".to_string())],
            wrappers: vec![("mangohud".to_string(), String::new())],
            ..Config::default()
        };
        assert_eq!(
            assemble(&cat, &config, None, &builder::Bins::default()),
            "PROTON_ENABLE_WAYLAND=1 mangohud %command%"
        );
    }

    #[test]
    fn extra_env_reaches_the_command() {
        let cat = Catalog::bundled();
        let config = Config {
            extra_env: "FOO=bar BAZ=1".to_string(),
            ..Config::default()
        };
        assert_eq!(assemble(&cat, &config, None, &builder::Bins::default()), "FOO=bar BAZ=1 %command%");
    }

    #[test]
    fn extra_env_honors_quotes() {
        // `split_whitespace` used to shear this into `FOO="a` + a dropped `b"`.
        assert_eq!(
            parse_extra_env("FOO=\"a b\" BAR=1"),
            vec![
                ("FOO".to_string(), "a b".to_string()),
                ("BAR".to_string(), "1".to_string()),
            ]
        );
        assert_eq!(
            parse_extra_env("WINEDLLOVERRIDES='winmm=n,b'"),
            vec![("WINEDLLOVERRIDES".to_string(), "winmm=n,b".to_string())]
        );
    }

    #[test]
    fn a_stale_env_key_survives_into_the_command() {
        // #62. This config used to assemble to a bare "%command%": the key is not
        // in the catalog, so `apply_lists` dropped it and the variable the user
        // saved simply stopped being emitted.
        let cat = Catalog::bundled();
        let config = Config {
            env: vec![("PROTON_ENABLE_NVAPI".to_string(), "1".to_string())],
            ..Config::default()
        };
        assert_eq!(
            assemble(&cat, &config, None, &builder::Bins::default()),
            "PROTON_ENABLE_NVAPI=1 %command%"
        );
    }

    #[test]
    fn the_custom_env_field_wins_over_a_stale_catalog_value() {
        // The leftover's value came from a catalog the app no longer has; what
        // the user typed into the visible field outranks it. Exactly one
        // assignment must be emitted, and it must be theirs.
        let cat = Catalog::bundled();
        let config = Config {
            env: vec![("PROTON_ENABLE_NVAPI".to_string(), "1".to_string())],
            extra_env: "PROTON_ENABLE_NVAPI=0".to_string(),
            ..Config::default()
        };
        assert_eq!(
            assemble(&cat, &config, None, &builder::Bins::default()),
            "PROTON_ENABLE_NVAPI=0 %command%"
        );
    }

    #[test]
    fn format_extra_env_round_trips_a_quoted_value() {
        // The old `unknown_env_string` joined pairs unquoted, so a recovered
        // `FOO="a b"` sheared into `FOO=a` plus a dropped `b"` on the next parse.
        let pairs = vec![
            ("FOO".to_string(), "a b".to_string()),
            ("BAR".to_string(), "1".to_string()),
        ];
        let s = format_extra_env(&pairs);
        assert_eq!(s, "FOO=\"a b\" BAR=1");
        assert_eq!(parse_extra_env(&s), pairs);
    }

    #[test]
    fn merge_into_extra_env_appends_only_what_is_missing() {
        let pairs = vec![
            ("A".to_string(), "1".to_string()),
            ("B".to_string(), "2".to_string()),
        ];
        assert_eq!(merge_into_extra_env("", &pairs), "A=1 B=2");
        assert_eq!(merge_into_extra_env("B=9", &pairs), "B=9 A=1");
        // Nothing to add must not perturb the field (not even its whitespace).
        assert_eq!(merge_into_extra_env("A=1 B=2", &pairs), "A=1 B=2");
        assert_eq!(merge_into_extra_env("", &[]), "");
    }

    #[test]
    fn umu_mode_uses_proton_path() {
        let cat = Catalog::bundled();
        let config = Config {
            umu: true,
            umu_exe: "/games/game.exe".to_string(),
            ..Config::default()
        };
        assert_eq!(
            assemble(&cat, &config, Some("/opt/proton"), &builder::Bins::default()),
            "GAMEID=umu-0 PROTONPATH=/opt/proton umu-run /games/game.exe"
        );
    }
}
