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

/// Rebuild `Options` from a `Config`'s catalog env/wrapper lists.
pub fn options_from_config(catalog: &Catalog, config: &Config) -> Options {
    let mut options = Options::from_catalog(catalog);
    store::apply_lists(catalog, &mut options, &config.env, &config.wrappers);
    options
}

/// Assemble the launch command for `config`. `proton_path` is the selected
/// runtime's install dir, used as `PROTONPATH` in umu mode and ignored otherwise.
pub fn assemble(catalog: &Catalog, config: &Config, proton_path: Option<&str>) -> String {
    let options = options_from_config(catalog, config);
    let (mut env, wrappers) = params::to_spec(catalog, &options);
    env.extend(parse_extra_env(&config.extra_env));

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
        )
    } else {
        builder::build_command(&env, &wrappers, &config.game_args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assemble_reproduces_all_combined_fixture() {
        // Byte-identical to builder::tests::all_combined, but driven the way the
        // app drives it. PROTON_USE_NTSYNC goes through `extra_env` because the
        // catalog dropped that key (Proton 11 kept only PROTON_NO_NTSYNC), and
        // `apply_lists` ignores env keys the catalog doesn't know.
        let cat = Catalog::bundled();
        let config = Config {
            extra_env: "PROTON_USE_NTSYNC=1".to_string(),
            wrappers: vec![
                ("gamemoderun".to_string(), String::new()),
                ("mangohud".to_string(), String::new()),
                ("gamescope".to_string(), "-f".to_string()),
            ],
            game_args: "--skip-launcher".to_string(),
            ..Config::default()
        };
        assert_eq!(
            assemble(&cat, &config, None),
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
            assemble(&cat, &config, None),
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
        assert_eq!(assemble(&cat, &config, None), "FOO=bar BAZ=1 %command%");
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
    fn umu_mode_uses_proton_path() {
        let cat = Catalog::bundled();
        let config = Config {
            umu: true,
            umu_exe: "/games/game.exe".to_string(),
            ..Config::default()
        };
        assert_eq!(
            assemble(&cat, &config, Some("/opt/proton")),
            "GAMEID=umu-0 PROTONPATH=/opt/proton umu-run /games/game.exe"
        );
    }
}
