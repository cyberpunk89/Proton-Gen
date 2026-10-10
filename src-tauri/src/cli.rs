//! Machine-readable command line: protongen's data as versioned JSON, for
//! scripts and for Nexus. Before this, Nexus re-derived it in Python: the
//! Heroic id hash, the config schema, a copy of the wrapper order, and
//! `state.toml` read directly. Everything here is read-only and uses the same
//! discovery, store and builder as the app, so the answers agree with what the
//! app shows.
//!
//! - `protongen --list --json`: runtimes and games, with ids, sources and
//!   Nexus slugs/aliases.
//! - `protongen --game-config <id> [--umu]`: one game's saved tuning, its built
//!   command and its resolved runtime. Steam mode unless `--umu`, or the saved
//!   config is umu.
//! - `protongen --catalog --json`: the wrappers in command order, plus env keys.
//!
//! Every document carries `"schema": SCHEMA`. Fields may be added within a
//! version; renaming or removing one bumps it.

use serde::Serialize;
use serde_json::{json, Value};

use crate::builder;
use crate::compose;
use crate::ipc::{self, GameDto, RuntimeDto};
use crate::params::{self, Catalog};
use crate::store::{Config, Store};

pub const SCHEMA: u32 = 1;

/// What the command line asked for. `None` from [`parse`] means "start the GUI".
#[derive(Debug, PartialEq)]
pub enum Request {
    /// The human-readable `--list` / `--scan` dump.
    Dump,
    ListJson,
    CatalogJson,
    GameConfig { app_id: u32, umu: bool },
}

pub fn parse(args: &[String]) -> Result<Option<Request>, String> {
    let has = |f: &str| args.iter().any(|a| a == f);
    let json = has("--json");
    if let Some(i) = args.iter().position(|a| a == "--game-config") {
        let id = args.get(i + 1).ok_or("--game-config needs an app id")?;
        let app_id = id.trim().parse::<u32>().map_err(|_| format!("not an app id: {id}"))?;
        return Ok(Some(Request::GameConfig { app_id, umu: has("--umu") }));
    }
    if has("--catalog") {
        return Ok(Some(Request::CatalogJson));
    }
    if has("--list") || has("--scan") {
        return Ok(Some(if json { Request::ListJson } else { Request::Dump }));
    }
    Ok(None)
}

fn print(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

/// Run a JSON request. `Dump` is the caller's (it lives with the GUI wiring).
pub fn run(req: Request) -> Result<(), String> {
    let (catalog, _) = Catalog::load();
    match req {
        Request::Dump => Err("dump is handled by lib::dump".into()),
        Request::CatalogJson => {
            print(&catalog_json(&catalog));
            Ok(())
        }
        Request::ListJson => {
            let store = Store::load();
            let d = ipc::scan_discovery(&catalog, &store.paths);
            print(&json!({
                "schema": SCHEMA,
                "steam_root": d.steam_root,
                "runtimes": d.runtimes.iter().map(runtime_json).collect::<Vec<_>>(),
                "games": d.games,
            }));
            Ok(())
        }
        Request::GameConfig { app_id, umu } => {
            let store = Store::load();
            let d = ipc::scan_discovery(&catalog, &store.paths);
            let game = d.games.iter().find(|g| g.app_id == app_id);
            let out = game_config_json(&catalog, &store, &d.runtimes, game, app_id, umu);
            print(&out);
            Ok(())
        }
    }
}

/// A runtime, plus `dir_name`: Nexus names Proton builds by folder, protongen
/// configs by `internal_name`, and the two differ for some builds.
fn runtime_json(r: &RuntimeDto) -> Value {
    let dir_name = std::path::Path::new(&r.path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    json!({
        "internal_name": r.internal_name,
        "display_name": r.display_name,
        "kind": r.kind,
        "path": r.path,
        "dir_name": dir_name,
    })
}

#[derive(Serialize)]
struct CatalogWrapper<'a> {
    key: &'a str,
    /// The binary it runs (`requires`, else the key).
    program: &'a str,
    /// Outer (low) to inner (high) — the builder's ordering.
    rank: u8,
}

fn catalog_json(catalog: &Catalog) -> Value {
    let mut wrappers: Vec<CatalogWrapper> = catalog
        .wrappers
        .iter()
        .filter_map(|def| {
            let w = params::wrapper_of(def, &def.default_value)?;
            Some(CatalogWrapper {
                key: &def.key,
                program: def.requires.as_deref().unwrap_or(&def.key),
                rank: w.rank(),
            })
        })
        .collect();
    wrappers.sort_by_key(|w| w.rank);
    json!({
        "schema": SCHEMA,
        "catalog": { "proton_cachyos_build": catalog.meta.proton_cachyos_build, "updated": catalog.meta.updated },
        "wrappers": wrappers,
        "env": catalog.envs.iter().map(|e| &e.key).collect::<Vec<_>>(),
    })
}

/// The saved config for `app_id`, falling back to the first absorbed mirror
/// that has one (the app copies it over on next start; a script shouldn't
/// have to wait for that).
fn saved_config<'a>(store: &'a Store, game: Option<&GameDto>, app_id: u32) -> Option<(&'a Config, u32)> {
    std::iter::once(app_id)
        .chain(game.map(|g| g.alias_ids.clone()).unwrap_or_default())
        .find_map(|id| store.game_memory.get(&id.to_string()).map(|c| (c, id)))
}

fn game_config_json(
    catalog: &Catalog,
    store: &Store,
    runtimes: &[RuntimeDto],
    game: Option<&GameDto>,
    app_id: u32,
    umu: bool,
) -> Value {
    let saved = saved_config(store, game, app_id);
    let Some((cfg, from_id)) = saved else {
        return json!({
            "schema": SCHEMA,
            "app_id": app_id,
            "known": game.is_some(),
            "game": game,
            "config": null,
            "command": null,
            "runtime": null,
        });
    };
    let mut cfg = cfg.clone();
    if umu && !cfg.umu {
        cfg.umu = true;
        if cfg.umu_exe.trim().is_empty() {
            cfg.umu_exe = game.and_then(|g| g.executable.clone()).unwrap_or_default();
        }
    }
    if cfg.umu && cfg.umu_wineprefix.trim().is_empty() {
        cfg.umu_wineprefix = game.and_then(|g| g.wine_prefix.clone()).unwrap_or_default();
    }
    let runtime = cfg
        .runtime
        .as_deref()
        .and_then(|name| runtimes.iter().find(|r| r.internal_name == name));
    let bins = builder::Bins::with_overrides(&store.paths.bins);
    let command = compose::assemble(catalog, &cfg, runtime.map(|r| r.path.as_str()), &bins);
    json!({
        "schema": SCHEMA,
        "app_id": app_id,
        "known": game.is_some(),
        "game": game,
        "saved_under": from_id,
        "config": cfg,
        "command": command,
        "runtime": runtime.map(runtime_json),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_routes_each_flag() {
        assert_eq!(parse(&argv(&["protongen"])), Ok(None));
        assert_eq!(parse(&argv(&["protongen", "--game", "10"])), Ok(None), "--game is the GUI's");
        assert_eq!(parse(&argv(&["protongen", "--list"])), Ok(Some(Request::Dump)));
        assert_eq!(parse(&argv(&["protongen", "--list", "--json"])), Ok(Some(Request::ListJson)));
        assert_eq!(parse(&argv(&["protongen", "--catalog", "--json"])), Ok(Some(Request::CatalogJson)));
        assert_eq!(
            parse(&argv(&["protongen", "--game-config", "3632060096", "--umu"])),
            Ok(Some(Request::GameConfig { app_id: 3_632_060_096, umu: true }))
        );
        assert!(parse(&argv(&["protongen", "--game-config"])).is_err());
        assert!(parse(&argv(&["protongen", "--game-config", "x"])).is_err());
    }

    #[test]
    fn catalog_wrappers_come_out_in_builder_order() {
        let v = catalog_json(&Catalog::bundled());
        let ranks: Vec<u64> = v["wrappers"].as_array().unwrap().iter().map(|w| w["rank"].as_u64().unwrap()).collect();
        assert!(ranks.windows(2).all(|p| p[0] <= p[1]));
        let keys: Vec<&str> = v["wrappers"].as_array().unwrap().iter().map(|w| w["key"].as_str().unwrap()).collect();
        let pos = |k| keys.iter().position(|x| *x == k).unwrap();
        assert!(pos("gamescope") < pos("gamemoderun"));
        assert!(pos("gamemoderun") < pos("mangohud"));
        assert_eq!(v["schema"], SCHEMA);
    }

    fn game(app_id: u32, aliases: Vec<u32>) -> GameDto {
        GameDto {
            app_id,
            name: "G".into(),
            source: "nexus".into(),
            executable: Some("/g/g.exe".into()),
            installed: true,
            last_played: None,
            playtime_minutes: None,
            heroic_id: None,
            install_dir: None,
            update_pending: false,
            last_updated: None,
            build_id: None,
            size_on_disk: None,
            art_url: None,
            nexus_slug: Some("g".into()),
            wine_prefix: Some("/g/pfx".into()),
            pinned_proton: None,
            alias_ids: aliases,
        }
    }

    #[test]
    fn game_config_falls_back_to_a_mirror_and_fills_the_prefix() {
        let mut store = Store::default();
        let mut cfg = Config::default();
        cfg.umu = true;
        cfg.umu_exe = "/g/g.exe".into();
        cfg.env = vec![("PROTON_ENABLE_WAYLAND".into(), "1".into())];
        store.game_memory.insert("77".into(), cfg);
        let g = game(10, vec![77]);
        let v = game_config_json(&Catalog::bundled(), &store, &[], Some(&g), 10, false);
        assert_eq!(v["saved_under"], 77);
        let cmd = v["command"].as_str().unwrap();
        assert!(cmd.starts_with("WINEPREFIX=/g/pfx "), "{cmd}");
        assert!(cmd.contains("PROTON_ENABLE_WAYLAND=1"), "{cmd}");
    }

    #[test]
    fn runtime_json_names_the_folder_nexus_matches_on() {
        let r = RuntimeDto {
            internal_name: "proton_experimental".into(),
            display_name: "Proton - Experimental".into(),
            kind: "steam".into(),
            path: "/s/steamapps/common/Proton - Experimental".into(),
        };
        assert_eq!(runtime_json(&r)["dir_name"], "Proton - Experimental");
    }

    #[test]
    fn game_config_with_nothing_saved_is_explicit_null() {
        let v = game_config_json(&Catalog::bundled(), &Store::default(), &[], None, 5, false);
        assert_eq!(v["known"], false);
        assert!(v["config"].is_null() && v["command"].is_null());
    }
}
