//! Compare the launch command protongen built against the one Steam actually
//! has set, and say whether they agree.
//!
//! **Why not a string compare.** Env ordering differs, gamescope arg whitespace
//! differs, and the two sides disagree about quoting: `parser::tokenize` strips
//! quotes while `builder::shell_quote` only *adds* them around whitespace. A raw
//! `==` would report "drifted" almost constantly. So both sides are run through
//! `parser::parse` and compared as normal forms.
//!
//! Normalisation, deliberately:
//!
//! - **env ordering is ignored.** Entries go into a `BTreeMap`, last assignment
//!   winning on duplicates, which is what a shell would do anyway. Two commands
//!   that set the same variables in a different order are the same command.
//! - **wrapper ordering is ignored** too — `builder::env_and_wrappers` already
//!   emits them in a canonical order, so position carries no information.
//! - **gamescope args and game args are whitespace-normalised**, since the
//!   builder's spacing is not something the user should have to reproduce.
//!
//! What is *not* normalised away: anything protongen cannot model. Those tokens
//! land in `unmodeled` (via `Parsed::unknown`) and force a `Drifted` verdict —
//! a launch string full of `prime-run`/`strangle` that happens to set no env
//! must never read as "in sync".

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;

use crate::builder::Wrapper;
use crate::compose;
use crate::params::Catalog;
use crate::parser::{self, Parsed};
use crate::store::Config;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiffStatus {
    /// Steam already has exactly this command.
    InSync,
    /// Steam has launch options, but not these ones.
    Drifted,
    /// Steam has no launch options set for this game at all.
    NotApplied,
    /// The built command is a umu invocation; Steam's launch options say
    /// nothing about it, so there is nothing to compare.
    Umu,
}

/// One key present on both sides with a different value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Change {
    /// Env var name, wrapper key, or `"game_args"`.
    pub key: String,
    pub current: String,
    pub built: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct LaunchDiff {
    pub status: DiffStatus,
    /// Keys in the built command that Steam does not have.
    pub added: Vec<String>,
    /// Keys Steam has that the built command does not.
    pub removed: Vec<String>,
    pub changed: Vec<Change>,
    /// Tokens protongen cannot represent (foreign wrappers, stray flags). Their
    /// presence alone is enough to call the command drifted.
    pub unmodeled: Vec<String>,
    /// The trailing game arguments, when the two sides disagree.
    pub game_args: Option<Change>,
}

/// Collapse a parse into `key -> value`: env vars plus wrappers, since a wrapper
/// is just another thing that is either present or not. Wrapper keys can't
/// collide with env keys — an env key always contains `=` before tokenizing and
/// the wrapper names are fixed lowercase words.
fn normal_form(p: &Parsed) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (k, v) in &p.env {
        // Last assignment wins, matching shell semantics.
        map.insert(k.clone(), v.clone());
    }
    for w in p.wrappers() {
        match w {
            Wrapper::Gamescope(args) => {
                map.insert("gamescope".to_string(), squash(&args));
            }
            Wrapper::Gamemoderun => {
                map.insert("gamemoderun".to_string(), String::new());
            }
            Wrapper::Mangohud => {
                map.insert("mangohud".to_string(), String::new());
            }
        }
    }
    map
}

/// Collapse runs of whitespace so `-W 2560  -H 1440` == `-W 2560 -H 1440`.
fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Compare a built launch command against the one currently set in Steam.
/// Pure: everything it needs is in the two strings.
pub fn compare(built: &str, current: &str) -> LaunchDiff {
    let b = parser::parse(built);
    let c = parser::parse(current);

    let bm = normal_form(&b);
    let cm = normal_form(&c);

    let added: Vec<String> = bm.keys().filter(|k| !cm.contains_key(*k)).cloned().collect();
    let removed: Vec<String> = cm.keys().filter(|k| !bm.contains_key(*k)).cloned().collect();
    let changed: Vec<Change> = bm
        .iter()
        .filter_map(|(k, bv)| {
            let cv = cm.get(k)?;
            (cv != bv).then(|| Change {
                key: k.clone(),
                current: cv.clone(),
                built: bv.clone(),
            })
        })
        .collect();

    // Both sides, deduped: in practice only the Steam side has any, but a
    // built command is not guaranteed free of them and silently hiding one
    // would be the exact bug #27 fixed.
    let unmodeled: Vec<String> = c
        .unknown
        .iter()
        .chain(&b.unknown)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let (bargs, cargs) = (squash(&b.game_args), squash(&c.game_args));
    let game_args = (bargs != cargs).then(|| Change {
        key: "game_args".to_string(),
        current: cargs,
        built: bargs,
    });

    let identical = added.is_empty()
        && removed.is_empty()
        && changed.is_empty()
        && unmodeled.is_empty()
        && game_args.is_none();

    let status = if b.umu {
        // Checked before NotApplied on purpose: for a umu command, "Steam has
        // no launch options" is not a to-do, it's irrelevant. Reporting
        // NotApplied would tell the user to go paste it into Steam.
        DiffStatus::Umu
    } else if current.trim().is_empty() {
        DiffStatus::NotApplied
    } else if identical {
        DiffStatus::InSync
    } else {
        DiffStatus::Drifted
    };

    LaunchDiff {
        status,
        added,
        removed,
        changed,
        unmodeled,
        game_args,
    }
}

/// One status per remembered game, for the library grid — the batch form of
/// [`compare`]. Pure: `launch_options` is passed in rather than read off
/// `AppState`, whose discovery snapshot is deliberately never refreshed by
/// `rescan` and would go stale after a library refresh.
///
/// `proton_path` is `None` throughout: it only feeds `PROTONPATH` in umu mode,
/// and umu configs short-circuit to [`DiffStatus::Umu`] before anything is
/// built.
///
/// No caveat: [`compose::assemble`] renders a config faithfully. It used to drop
/// env keys the catalog no longer knew, so a config remembered before a catalog
/// refresh assembled into a command missing them and could compare equal to
/// Steam while being nothing of the sort — which this function papered over by
/// force-downgrading such configs to `Drifted`. #62 fixed that at the source:
/// the keys now survive into the command, so the verdict falls out of the
/// ordinary comparison. (Unknown *wrapper* keys are still dropped — see
/// [`store::options_from_lists`] for why they cannot drift the same way.)
pub fn statuses(
    catalog: &Catalog,
    memory: &BTreeMap<String, Config>,
    launch_options: &HashMap<String, String>,
) -> HashMap<String, DiffStatus> {
    memory
        .iter()
        .map(|(appid, config)| {
            let status = if config.umu {
                // Steam's launch options say nothing about a umu command.
                DiffStatus::Umu
            } else {
                let built = compose::assemble(catalog, config, None);
                let current = launch_options.get(appid).map(String::as_str).unwrap_or("");
                compare(&built, current).status
            };
            (appid.clone(), status)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_order_and_quoting_do_not_count_as_drift() {
        let d = compare(
            "DXVK_HUD=fps PROTON_ENABLE_WAYLAND=1 mangohud %command%",
            "mangohud PROTON_ENABLE_WAYLAND=1 DXVK_HUD=\"fps\" %command%",
        );
        assert_eq!(d.status, DiffStatus::InSync);
        assert!(d.added.is_empty() && d.removed.is_empty() && d.changed.is_empty());
    }

    #[test]
    fn gamescope_arg_whitespace_does_not_count_as_drift() {
        let d = compare(
            "gamescope -W 2560 -H 1440 -f -- %command%",
            "gamescope  -W   2560 -H 1440   -f -- %command%",
        );
        assert_eq!(d.status, DiffStatus::InSync);
    }

    #[test]
    fn duplicate_assignment_takes_the_last_one() {
        // What the shell would do; the two sides agree on the effective value.
        let d = compare("DXVK_HUD=fps %command%", "DXVK_HUD=full DXVK_HUD=fps %command%");
        assert_eq!(d.status, DiffStatus::InSync);
    }

    #[test]
    fn reports_added_removed_and_changed() {
        let d = compare(
            "DXVK_HUD=fps PROTON_LOG=1 gamescope -f -- %command%",
            "DXVK_HUD=full mangohud %command%",
        );
        assert_eq!(d.status, DiffStatus::Drifted);
        assert_eq!(d.added, vec!["PROTON_LOG", "gamescope"]);
        assert_eq!(d.removed, vec!["mangohud"]);
        assert_eq!(
            d.changed,
            vec![Change {
                key: "DXVK_HUD".to_string(),
                current: "full".to_string(),
                built: "fps".to_string(),
            }]
        );
    }

    #[test]
    fn game_args_are_compared_separately() {
        let d = compare("%command% --skip-launcher", "%command% -windowed");
        assert_eq!(d.status, DiffStatus::Drifted);
        assert_eq!(
            d.game_args,
            Some(Change {
                key: "game_args".to_string(),
                current: "-windowed".to_string(),
                built: "--skip-launcher".to_string(),
            })
        );
        // Whitespace alone is not a difference.
        assert!(compare("%command% -a  -b", "%command%   -a -b").game_args.is_none());
    }

    #[test]
    fn foreign_wrappers_force_drift_even_with_matching_env() {
        // The env sets match exactly; only the unmodelable token differs. Before
        // #27 this parsed away to nothing and read as in-sync.
        let d = compare("DXVK_HUD=fps %command%", "DXVK_HUD=fps prime-run %command%");
        assert_eq!(d.status, DiffStatus::Drifted);
        assert_eq!(d.unmodeled, vec!["prime-run"]);
        assert!(d.added.is_empty() && d.removed.is_empty() && d.changed.is_empty());
    }

    #[test]
    fn empty_current_is_not_applied() {
        let d = compare("DXVK_HUD=fps %command%", "");
        assert_eq!(d.status, DiffStatus::NotApplied);
        // The payload still describes what applying it would add.
        assert_eq!(d.added, vec!["DXVK_HUD"]);

        assert_eq!(compare("%command%", "   ").status, DiffStatus::NotApplied);
    }

    #[test]
    fn a_umu_command_is_never_compared_against_steam() {
        let d = compare(
            "GAMEID=umu-0 PROTONPATH=/opt/proton umu-run /games/game.exe",
            "DXVK_HUD=fps %command%",
        );
        assert_eq!(d.status, DiffStatus::Umu);
        // Empty Steam options don't downgrade it to NotApplied either — there is
        // nothing for the user to paste anywhere.
        assert_eq!(
            compare("GAMEID=umu-0 umu-run /games/game.exe", "").status,
            DiffStatus::Umu
        );
    }

    #[test]
    fn a_bare_command_placeholder_matches_the_default_config() {
        // The command a freshly-reset builder produces.
        assert_eq!(compare("%command%", "%command%").status, DiffStatus::InSync);
    }

    // ------------------------------- statuses --------------------------------

    fn memory(entries: &[(&str, Config)]) -> BTreeMap<String, Config> {
        entries
            .iter()
            .map(|(id, c)| (id.to_string(), c.clone()))
            .collect()
    }

    fn options(entries: &[(&str, &str)]) -> HashMap<String, String> {
        entries
            .iter()
            .map(|(id, v)| (id.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn statuses_covers_every_remembered_game() {
        let cat = Catalog::bundled();
        let wayland = Config {
            env: vec![("PROTON_ENABLE_WAYLAND".to_string(), "1".to_string())],
            ..Config::default()
        };
        let mem = memory(&[
            // Exactly what Steam has.
            ("1", wayland.clone()),
            // Steam has something else.
            ("2", wayland.clone()),
            // Remembered, but never pasted into Steam.
            ("3", wayland.clone()),
            // umu: Steam's launch options are irrelevant.
            (
                "4",
                Config {
                    umu: true,
                    umu_exe: "/games/game.exe".to_string(),
                    ..Config::default()
                },
            ),
        ]);
        let opts = options(&[
            ("1", "PROTON_ENABLE_WAYLAND=1 %command%"),
            ("2", "mangohud %command%"),
            ("4", "whatever %command%"),
            // A game with launch options but no remembered config is not ours
            // to report on.
            ("9", "PROTON_ENABLE_WAYLAND=1 %command%"),
        ]);

        let got = statuses(&cat, &mem, &opts);
        assert_eq!(got.len(), 4, "one entry per remembered game, and no more");
        assert_eq!(got["1"], DiffStatus::InSync);
        assert_eq!(got["2"], DiffStatus::Drifted);
        assert_eq!(got["3"], DiffStatus::NotApplied);
        assert_eq!(got["4"], DiffStatus::Umu);
        assert!(!got.contains_key("9"));
    }

    #[test]
    fn an_absolute_wrapper_path_does_not_read_as_drift() {
        // #66. `prime-run` in `foreign_wrappers_force_drift_even_with_matching_env`
        // *should* drift — it is a program we don't model. `/usr/bin/mangohud` is
        // the same program we emit, named by path, and used to drift for no
        // reason other than string equality.
        let d = compare("mangohud %command%", "/usr/bin/mangohud %command%");
        assert_eq!(d.status, DiffStatus::InSync);
        assert!(d.unmodeled.is_empty(), "got: {:?}", d.unmodeled);
    }

    #[test]
    fn a_stale_env_key_is_compared_rather_than_dropped() {
        // #62. `store::apply_lists` used to discard env keys the catalog no
        // longer knows, so this config assembled to a bare "%command%" and
        // compared *equal* to Steam — a confident InSync verdict about a
        // variable that had vanished in a catalog refresh. `statuses` papered
        // over that by force-downgrading such configs to Drifted, which bought
        // honesty in one direction and lost it in the other: a config that
        // genuinely matched Steam could never say so.
        //
        // The key now survives into the command, so both verdicts fall out of
        // the ordinary comparison.
        let cat = Catalog::bundled();
        let stale = Config {
            env: vec![("PROTON_ENABLE_NVAPI".to_string(), "1".to_string())],
            ..Config::default()
        };
        assert!(
            !cat.envs.iter().any(|e| e.key == "PROTON_ENABLE_NVAPI"),
            "fixture must name a key the bundled catalog really lacks"
        );
        assert_eq!(
            compose::assemble(&cat, &stale, None),
            "PROTON_ENABLE_NVAPI=1 %command%"
        );

        // Steam has nothing set for it: genuinely drifted.
        let got = statuses(
            &cat,
            &memory(&[("1", stale.clone())]),
            &options(&[("1", "%command%")]),
        );
        assert_eq!(got["1"], DiffStatus::Drifted);

        // Steam has exactly it: genuinely in sync. This is the false negative
        // the old workaround caused, and nothing used to test it.
        let got = statuses(
            &cat,
            &memory(&[("1", stale)]),
            &options(&[("1", "PROTON_ENABLE_NVAPI=1 %command%")]),
        );
        assert_eq!(got["1"], DiffStatus::InSync);
    }

    #[test]
    fn a_stale_env_key_still_reads_as_not_applied_against_empty_steam_options() {
        // "Steam has nothing set" is true regardless of where the key lives.
        let cat = Catalog::bundled();
        let stale = Config {
            env: vec![("PROTON_ENABLE_NVAPI".to_string(), "1".to_string())],
            ..Config::default()
        };
        let got = statuses(&cat, &memory(&[("1", stale)]), &HashMap::new());
        assert_eq!(got["1"], DiffStatus::NotApplied);
    }
}
