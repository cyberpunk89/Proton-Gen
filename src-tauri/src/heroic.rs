//! Heroic Games Launcher integration: read-only discovery of *sideloaded*
//! (exe-installed) games, plus a **sanctioned write path** that injects
//! protongen's env vars + wrappers into a game's per-game config.
//!
//! Heroic does not consume a launch string the way Steam does — it reads
//! structured per-game JSON under `GamesConfig/<app_name>.json`. So for a Heroic
//! game "copy the command" is useless; the useful action is to write the tuned
//! env/wrappers straight into that file. This is the one place protongen writes
//! outside its own `state.toml`, and it does so conservatively: back up first,
//! preserve every key it doesn't own, write atomically.

use std::path::PathBuf;

use serde::Deserialize;

use crate::builder::{Bins, Wrapper};

/// `$XDG_CONFIG_HOME/heroic` (or `~/.config/heroic`). Mirrors
/// [`crate::params::config_dir`] but for Heroic's config tree, not protongen's.
fn config_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(xdg).join("heroic"));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/heroic"))
}

/// `GamesConfig/<app_name>.json` under the Heroic config dir.
fn game_config_path(app_name: &str) -> Option<PathBuf> {
    Some(config_dir()?.join("GamesConfig").join(format!("{app_name}.json")))
}

// ----------------------------- discovery -----------------------------

#[derive(Deserialize)]
struct SideloadLibrary {
    #[serde(default)]
    games: Vec<SideloadEntry>,
}

#[derive(Deserialize)]
struct SideloadEntry {
    #[serde(default)]
    runner: String,
    #[serde(default)]
    app_name: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    install: SideloadInstall,
    #[serde(default)]
    is_installed: bool,
    /// Vertical box art Heroic shows in its own library grid — a `file://` path
    /// (Heroic's bundled placeholder, or an image the user picked when adding
    /// the sideload) or a remote URL (commonly SteamGridDB, when the user
    /// searched for cover art in Heroic's UI).
    #[serde(default)]
    art_cover: Option<String>,
    /// Square art, Heroic's fallback when a game has no `art_cover`.
    #[serde(default)]
    art_square: Option<String>,
}

#[derive(Default, Deserialize)]
struct SideloadInstall {
    #[serde(default)]
    executable: Option<String>,
}

/// A sideloaded Heroic game — the only kind we scan (not GOG/Epic).
pub struct HeroicGame {
    /// Heroic's stable per-game id (base62), and the `GamesConfig` filename stem.
    pub app_name: String,
    pub title: String,
    /// Target exe, used to prefill umu mode. `None` if Heroic recorded none.
    pub executable: Option<String>,
    pub installed: bool,
    /// Box art Heroic has for this game (`art_cover`, falling back to
    /// `art_square`) — a `file://` path or a remote URL. Passed through to
    /// `art::fetch` as a resolution hint, since a sideloaded game has no Steam
    /// appid a local/CDN cache lookup could key off. `None` when the user added
    /// the game without picking cover art.
    pub art: Option<String>,
}

/// Sideloaded games from `sideload_apps/library.json`. Any absence — no Heroic
/// installed, no sideloaded games, an unreadable or malformed file — yields an
/// empty list rather than an error: Heroic simply isn't part of this setup.
pub fn list_sideloaded() -> Vec<HeroicGame> {
    let Some(dir) = config_dir() else {
        return Vec::new();
    };
    let path = dir.join("sideload_apps").join("library.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(lib) = serde_json::from_str::<SideloadLibrary>(&raw) else {
        return Vec::new();
    };
    lib.games
        .into_iter()
        // Defensive: sideload_apps should only hold `sideload` runners, but a
        // stray gog/epic entry must never leak into the sideloaded scan.
        .filter(|g| g.runner == "sideload" && !g.app_name.is_empty())
        .map(entry_to_game)
        .collect()
}

/// `SideloadEntry` -> `HeroicGame`. Split out from [`list_sideloaded`] (which
/// owns the filesystem read) so the field mapping can be unit-tested against a
/// hand-built entry, with no XDG/filesystem plumbing involved.
fn entry_to_game(g: SideloadEntry) -> HeroicGame {
    let executable =
        g.install.executable.map(|e| e.trim().to_string()).filter(|e| !e.is_empty());
    let art =
        g.art_cover.or(g.art_square).map(|a| a.trim().to_string()).filter(|a| !a.is_empty());
    HeroicGame { app_name: g.app_name, title: g.title, executable, installed: g.is_installed, art }
}

// ------------------------ running-process detection ------------------------

/// Does `/proc/<pid>/comm`'s content look like Heroic's own process?
///
/// `comm` is the kernel's idea of the executable's name (truncated to 15
/// bytes), trimmed of its trailing newline — for Heroic's `/opt/Heroic/heroic`
/// binary that's simply `"heroic"`. Matched case-insensitively as a substring
/// so a wrapped/renamed launch (`heroic-bin`, `Heroic`) still counts. Split out
/// from [`is_running`] (which owns the `/proc` walk) so the match rule is
/// unit-testable without a live process tree.
fn comm_matches(comm: &str) -> bool {
    comm.trim().to_ascii_lowercase().contains("heroic")
}

/// Best-effort check for a running Heroic process, so the "Apply to Heroic"
/// confirmation can warn *before* writing rather than let the user discover
/// after the fact that Heroic silently overwrote the injection on exit (Heroic
/// caches a game's settings in memory when it starts, and flushes that stale
/// copy back to `GamesConfig/<app_name>.json` on its own — with none of what
/// protongen just wrote). Scans `/proc/<pid>/comm` for every numeric entry
/// under `/proc`; any unreadable entry (a process that exited mid-scan, a
/// permission error) is skipped rather than failed, matching the tolerant
/// style of the rest of this module's discovery. Returns `false` on any
/// platform without a `/proc` (nothing here is Linux-specific by name, but the
/// scan itself is).
pub fn is_running() -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) else {
            continue;
        };
        if comm_matches(&comm) {
            return true;
        }
    }
    false
}

// ----------------------------- injection -----------------------------

/// What a successful [`inject`] wrote, for the UI toast.
#[derive(Clone, Debug, serde::Serialize)]
pub struct InjectResult {
    pub config_path: String,
    pub backup_path: String,
}

/// Merge protongen's resolved `env` + `wrappers` into a parsed Heroic game
/// config, returning the new document. **Pure** — the tested core.
///
/// protongen owns exactly four keys: `enviromentOptions` (Heroic's own
/// misspelling) and `wrapperOptions` are replaced wholesale, and
/// `showMangohud`/`useGameMode` are written as booleans reflecting the current
/// selection (both `true` *and* `false`, so toggling a wrapper off removes it and
/// re-injection is idempotent). Every other key — `wineVersion`, `winePrefix`,
/// the fsync/esync toggles, top-level `version`/`explicit` — is left untouched.
pub fn apply_to_config(
    mut root: serde_json::Value,
    app_name: &str,
    env: &[(String, String)],
    wrappers: &[Wrapper],
    bins: &Bins,
) -> serde_json::Value {
    use serde_json::{json, Value};

    if !root.is_object() {
        root = json!({});
    }
    let top = root.as_object_mut().expect("root is an object");

    let entry = top.entry(app_name.to_string()).or_insert_with(|| json!({}));
    if !entry.is_object() {
        *entry = json!({});
    }
    let game = entry.as_object_mut().expect("entry is an object");

    // Environment variables -> enviromentOptions (wholesale).
    let env_arr: Vec<Value> = env
        .iter()
        .map(|(k, v)| json!({ "key": k, "value": v }))
        .collect();
    game.insert("enviromentOptions".to_string(), Value::Array(env_arr));

    // Wrappers -> native toggles where Heroic has them, wrapperOptions otherwise.
    let mut mangohud = false;
    let mut gamemode = false;
    let mut wrapper_opts: Vec<Value> = Vec::new();
    for w in wrappers {
        match w {
            Wrapper::Mangohud => mangohud = true,
            Wrapper::Gamemoderun => gamemode = true,
            // Heroic has no native game-performance toggle, so pass it as a
            // generic prefix wrapper it runs the game through.
            Wrapper::GamePerformance => {
                wrapper_opts.push(json!({ "exe": "game-performance", "args": "" }));
            }
            Wrapper::Gamescope(args) => {
                let args = args.trim();
                // Trailing `--` so Heroic's `exe args %command%` composition
                // yields `gamescope <args> -- <game>`, matching the Steam builder.
                let composed = if args.is_empty() {
                    "--".to_string()
                } else {
                    format!("{args} --")
                };
                wrapper_opts.push(json!({ "exe": bins.gamescope, "args": composed }));
            }
        }
    }
    game.insert("showMangohud".to_string(), Value::Bool(mangohud));
    game.insert("useGameMode".to_string(), Value::Bool(gamemode));
    game.insert("wrapperOptions".to_string(), Value::Array(wrapper_opts));

    root
}

/// Write `env` + `wrappers` into the Heroic game's `GamesConfig/<app_name>.json`.
/// Backs the file up first and writes atomically. Impure; thin.
pub fn inject(
    app_name: &str,
    env: &[(String, String)],
    wrappers: &[Wrapper],
    bins: &Bins,
) -> Result<InjectResult, String> {
    let path =
        game_config_path(app_name).ok_or_else(|| "Heroic config directory not found".to_string())?;

    // A game the user never opened in Heroic has no config yet. Creating a
    // partial one would omit `wineVersion`/`winePrefix` and could break launch,
    // so ask the user to let Heroic create it first.
    if !path.exists() {
        return Err(
            "This game has no Heroic config yet. Open its Settings in Heroic once to create it, \
             then try again."
                .to_string(),
        );
    }

    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    let root: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Heroic config is not valid JSON ({}): {e}", path.display()))?;

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("game.json");
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = path.with_file_name(format!("{file_name}.protongen-{ts}.bak"));
    std::fs::write(&backup, &raw)
        .map_err(|e| format!("Could not write backup {}: {e}", backup.display()))?;

    let patched = apply_to_config(root, app_name, env, wrappers, bins);
    let out = serde_json::to_string_pretty(&patched)
        .map_err(|e| format!("Could not serialize Heroic config: {e}"))?;

    // Atomic replace: temp in the same dir, then rename over the original.
    let tmp = path.with_file_name(format!("{file_name}.protongen-tmp"));
    std::fs::write(&tmp, out.as_bytes())
        .map_err(|e| format!("Could not write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("Could not replace {}: {e}", path.display()))?;

    Ok(InjectResult {
        config_path: path.display().to_string(),
        backup_path: backup.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn comm_matches_heroics_own_process_name() {
        assert!(comm_matches("heroic\n"));
        assert!(comm_matches("Heroic"));
        // A wrapped/renamed launch still counts.
        assert!(comm_matches("heroic-bin\n"));
    }

    #[test]
    fn comm_matches_rejects_unrelated_processes() {
        assert!(!comm_matches("steam\n"));
        assert!(!comm_matches("gamescope"));
        assert!(!comm_matches(""));
    }

    fn base_config() -> serde_json::Value {
        // A realistic Heroic game config: the keys protongen must not disturb.
        json!({
            "7Hm5": {
                "enableFsync": true,
                "enableEsync": false,
                "wineVersion": { "bin": "/opt/proton", "name": "proton-cachyos", "type": "proton" },
                "winePrefix": "/home/u/Games/Heroic/Prefixes/shared",
                "showMangohud": true,
                "useGameMode": true,
                "enviromentOptions": [ { "key": "OLD", "value": "1" } ],
                "wrapperOptions": []
            },
            "version": "v0",
            "explicit": true
        })
    }

    #[test]
    fn preserves_unknown_keys() {
        let out = apply_to_config(base_config(), "7Hm5", &[], &[], &Bins::default());
        let game = &out["7Hm5"];
        assert_eq!(game["enableFsync"], json!(true));
        assert_eq!(game["enableEsync"], json!(false));
        assert_eq!(game["wineVersion"]["name"], json!("proton-cachyos"));
        assert_eq!(game["winePrefix"], json!("/home/u/Games/Heroic/Prefixes/shared"));
        // Top-level metadata survives too.
        assert_eq!(out["version"], json!("v0"));
        assert_eq!(out["explicit"], json!(true));
    }

    #[test]
    fn writes_env_with_heroics_misspelled_key() {
        let env = vec![
            ("DXVK_HUD".to_string(), "fps".to_string()),
            ("PROTON_USE_NTSYNC".to_string(), "1".to_string()),
        ];
        let out = apply_to_config(base_config(), "7Hm5", &env, &[], &Bins::default());
        assert_eq!(
            out["7Hm5"]["enviromentOptions"],
            json!([
                { "key": "DXVK_HUD", "value": "fps" },
                { "key": "PROTON_USE_NTSYNC", "value": "1" },
            ])
        );
    }

    #[test]
    fn maps_wrappers_to_native_toggles_and_options() {
        let bins = Bins {
            gamescope: "gamescope-git".to_string(),
            ..Bins::default()
        };
        let wrappers = vec![
            Wrapper::Mangohud,
            Wrapper::Gamemoderun,
            Wrapper::Gamescope("-f -W 2560".to_string()),
        ];
        let out = apply_to_config(base_config(), "7Hm5", &[], &wrappers, &bins);
        let game = &out["7Hm5"];
        assert_eq!(game["showMangohud"], json!(true));
        assert_eq!(game["useGameMode"], json!(true));
        assert_eq!(
            game["wrapperOptions"],
            json!([ { "exe": "gamescope-git", "args": "-f -W 2560 --" } ])
        );
    }

    #[test]
    fn gamescope_without_args_still_gets_separator() {
        let out = apply_to_config(
            base_config(),
            "7Hm5",
            &[],
            &[Wrapper::Gamescope(String::new())],
            &Bins::default(),
        );
        assert_eq!(
            out["7Hm5"]["wrapperOptions"],
            json!([ { "exe": "gamescope", "args": "--" } ])
        );
    }

    #[test]
    fn empty_selection_clears_arrays_and_sets_toggles_false() {
        // Base config had showMangohud/useGameMode true and an OLD env var.
        let out = apply_to_config(base_config(), "7Hm5", &[], &[], &Bins::default());
        let game = &out["7Hm5"];
        assert_eq!(game["showMangohud"], json!(false));
        assert_eq!(game["useGameMode"], json!(false));
        assert_eq!(game["enviromentOptions"], json!([]));
        assert_eq!(game["wrapperOptions"], json!([]));
    }

    #[test]
    fn is_idempotent() {
        let env = vec![("DXVK_HUD".to_string(), "fps".to_string())];
        let wrappers = vec![Wrapper::Mangohud, Wrapper::Gamescope("-f".to_string())];
        let once = apply_to_config(base_config(), "7Hm5", &env, &wrappers, &Bins::default());
        let twice = apply_to_config(once.clone(), "7Hm5", &env, &wrappers, &Bins::default());
        assert_eq!(once, twice);
    }

    #[test]
    fn creates_entry_when_absent() {
        // A config file that exists but has no object for this app_name.
        let root = json!({ "version": "v0" });
        let out = apply_to_config(root, "NewGame", &[], &[Wrapper::Mangohud], &Bins::default());
        assert_eq!(out["NewGame"]["showMangohud"], json!(true));
        assert_eq!(out["version"], json!("v0"));
    }

    #[test]
    fn list_sideloaded_parses_and_filters_runner() {
        let raw = r#"{
            "games": [
                { "runner": "sideload", "app_name": "abc", "title": "Crimson Desert",
                  "install": { "executable": "/games/cd/CrimsonDesert.exe" }, "is_installed": true,
                  "art_cover": "https://cdn2.steamgriddb.com/grid/abc.png",
                  "art_square": "https://cdn2.steamgriddb.com/grid/abc-square.png" },
                { "runner": "gog", "app_name": "xyz", "title": "Not Sideloaded",
                  "install": { "executable": "/games/x.exe" }, "is_installed": true }
            ]
        }"#;
        let lib: SideloadLibrary = serde_json::from_str(raw).unwrap();
        let games: Vec<_> = lib
            .games
            .into_iter()
            .filter(|g| g.runner == "sideload" && !g.app_name.is_empty())
            .collect();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].app_name, "abc");
        assert_eq!(games[0].title, "Crimson Desert");
        assert_eq!(
            games[0].install.executable.as_deref(),
            Some("/games/cd/CrimsonDesert.exe")
        );
        // art_cover wins over art_square when both are present.
        assert_eq!(
            games[0].art_cover.as_deref(),
            Some("https://cdn2.steamgriddb.com/grid/abc.png")
        );
    }

    #[test]
    fn entry_to_game_prefers_art_cover_falls_back_to_art_square() {
        let mut e = SideloadEntry {
            runner: "sideload".to_string(),
            app_name: "a".to_string(),
            title: "Has cover".to_string(),
            install: SideloadInstall::default(),
            is_installed: true,
            art_cover: Some("file:///covers/a.jpg".to_string()),
            art_square: Some("file:///covers/a-sq.jpg".to_string()),
        };
        assert_eq!(entry_to_game(e).art.as_deref(), Some("file:///covers/a.jpg"));

        e = SideloadEntry {
            app_name: "b".to_string(),
            art_cover: None,
            art_square: Some("file:///covers/b-sq.jpg".to_string()),
            ..blank_entry()
        };
        assert_eq!(entry_to_game(e).art.as_deref(), Some("file:///covers/b-sq.jpg"));

        e = SideloadEntry { app_name: "c".to_string(), ..blank_entry() };
        assert_eq!(entry_to_game(e).art, None);
    }

    fn blank_entry() -> SideloadEntry {
        SideloadEntry {
            runner: "sideload".to_string(),
            app_name: String::new(),
            title: String::new(),
            install: SideloadInstall::default(),
            is_installed: false,
            art_cover: None,
            art_square: None,
        }
    }
}
