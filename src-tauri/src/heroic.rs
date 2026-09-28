//! Heroic Games Launcher integration: read-only discovery of Heroic's games —
//! both manually-added *sideloaded* exes and titles installed through Heroic's
//! native stores (Epic via legendary, GOG, Amazon via nile) — plus a
//! **sanctioned write path** that injects protongen's env vars + wrappers into
//! a game's per-game config.
//!
//! Heroic does not consume a launch string the way Steam does — it reads
//! structured per-game JSON under `GamesConfig/<app_name>.json`. So for a Heroic
//! game "copy the command" is useless; the useful action is to write the tuned
//! env/wrappers straight into that file. This is the one place protongen writes
//! outside its own `state.toml`, and it does so conservatively: back up first,
//! preserve every key it doesn't own, write atomically.

use std::collections::HashMap;
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

/// `GamesConfig/<app_name>.json` under the Heroic config dir. `None` for an
/// `app_name` that isn't a plain file stem, so it can never name a path
/// outside `GamesConfig/`.
fn game_config_path(app_name: &str) -> Option<PathBuf> {
    if !is_valid_app_name(app_name) {
        return None;
    }
    Some(config_dir()?.join("GamesConfig").join(format!("{app_name}.json")))
}

/// Heroic app names are store ids — sideload/Legendary base62, GOG digits,
/// Amazon `amzn1.adg.product.<uuid>` — so letters, digits, `.`, `_`, `-` and
/// no leading dot covers them all while ruling out `/` and `..`.
fn is_valid_app_name(app_name: &str) -> bool {
    !app_name.is_empty()
        && !app_name.starts_with('.')
        && app_name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

// ----------------------------- discovery -----------------------------

/// Shared shape of every Heroic game-list file this module reads: the
/// sideload library (`sideload_apps/library.json`, top-level key `games`) and
/// each native store's cached library under `store_cache/`
/// (`legendary_library.json`/`nile_library.json` use `library`,
/// `gog_library.json` uses `games`) — otherwise identical `GameInfo` records.
#[derive(Deserialize)]
struct LibraryFile {
    #[serde(alias = "library", default)]
    games: Vec<LibraryEntry>,
}

#[derive(Deserialize)]
struct LibraryEntry {
    #[serde(default)]
    runner: String,
    #[serde(default)]
    app_name: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    install: LibraryInstall,
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
struct LibraryInstall {
    /// The sideload library stores an absolute path here directly; the native
    /// stores store just the exe filename, resolved against `install_path`.
    #[serde(default)]
    executable: Option<String>,
    #[serde(default)]
    install_path: Option<String>,
    #[serde(default)]
    is_dlc: bool,
}

/// A Heroic game — sideloaded or a native GOG/Epic/Amazon install.
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
    // Defensive: sideload_apps should only hold `sideload` runners, but a
    // stray gog/epic entry must never leak into the sideloaded scan. Every
    // sideload entry the user added is shown regardless of `is_installed`
    // (it's already a small, manually-curated list, unlike a native store's
    // full owned-games library).
    parse_library(&raw, "sideload", false)
}

/// Installed games from Heroic's native stores — Epic (via legendary), GOG,
/// and Amazon (via nile) — as opposed to [`list_sideloaded`]'s manually-added
/// exes. Heroic caches each store's *entire owned library* (installed or not)
/// under `store_cache/<store>_library.json`, so unlike sideload entries this
/// filters to `is_installed` (and drops DLC records): showing every
/// not-yet-installed Epic game would flood the picker with titles that have
/// nothing local to tune. Any absence — no Heroic, no store, an unreadable or
/// malformed cache file — yields an empty list for that store rather than an
/// error, matching [`list_sideloaded`]'s tolerance.
pub fn list_installed_native() -> Vec<HeroicGame> {
    let Some(cache_dir) = config_dir().map(|d| d.join("store_cache")) else {
        return Vec::new();
    };
    const SOURCES: &[(&str, &str)] = &[
        ("legendary_library.json", "legendary"),
        ("gog_library.json", "gog"),
        ("nile_library.json", "nile"),
    ];
    SOURCES
        .iter()
        .flat_map(|(file, runner)| {
            let raw = std::fs::read_to_string(cache_dir.join(file)).unwrap_or_default();
            parse_library(&raw, runner, true)
        })
        .collect()
}

/// Parse a `LibraryFile` (sideload library or a native store's cached
/// library), keeping only entries for `runner` that aren't DLC, and — when
/// `require_installed` — that Heroic reports as installed. Split from the
/// callers (which own the filesystem read) so parsing is unit-testable
/// against hand-built JSON with no XDG/filesystem plumbing involved.
fn parse_library(raw: &str, runner: &str, require_installed: bool) -> Vec<HeroicGame> {
    let Ok(lib) = serde_json::from_str::<LibraryFile>(raw) else {
        return Vec::new();
    };
    lib.games
        .into_iter()
        .filter(|g| g.runner == runner && !g.app_name.is_empty() && !g.install.is_dlc)
        .filter(|g| !require_installed || g.is_installed)
        .map(entry_to_game)
        .collect()
}

/// `LibraryEntry` -> `HeroicGame`. Split out from [`parse_library`] so the
/// field mapping can be unit-tested against a hand-built entry.
fn entry_to_game(g: LibraryEntry) -> HeroicGame {
    let executable = resolve_executable(g.install.install_path.as_deref(), g.install.executable.as_deref());
    let art =
        g.art_cover.or(g.art_square).map(|a| a.trim().to_string()).filter(|a| !a.is_empty());
    HeroicGame { app_name: g.app_name, title: g.title, executable, installed: g.is_installed, art }
}

/// Resolve an install's target exe to an absolute-ish path. The sideload
/// library stores the full path directly in `executable`; the native stores
/// (legendary/gog/nile) store just the exe filename, relative to
/// `install_path`, so those two must be joined.
fn resolve_executable(install_path: Option<&str>, executable: Option<&str>) -> Option<String> {
    let exe = executable.map(str::trim).filter(|e| !e.is_empty())?;
    if exe.starts_with('/') {
        return Some(exe.to_string());
    }
    match install_path.map(str::trim).filter(|p| !p.is_empty()) {
        Some(dir) => Some(format!("{}/{exe}", dir.trim_end_matches('/'))),
        None => Some(exe.to_string()),
    }
}

// ------------------------------ play stats ----------------------------------

/// One game's tracked play activity, from Heroic's own `store/timestamp.json` —
/// the file Heroic itself uses for last-played/playtime across *every* runner
/// it manages, sideloaded games included. Mirrors `steamcfg::AppUserCfg`'s
/// last_played/playtime_minutes pair, minus `launch_options` (no equivalent
/// here) and minus multi-account merging (this is one flat file, not one per
/// Steam user).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayStats {
    /// Unix seconds, parsed from `lastPlayed`. `None` if absent or unparseable.
    pub last_played: Option<u64>,
    /// `totalPlayed` — Heroic accumulates this in minutes, the same unit as
    /// Steam's `Playtime` (confirmed against Heroic's own `launcher.ts`).
    pub playtime_minutes: Option<u32>,
}

/// `store/timestamp.json` under the Heroic config dir.
fn timestamp_path() -> Option<PathBuf> {
    Some(config_dir()?.join("store").join("timestamp.json"))
}

#[derive(Deserialize)]
struct TimestampEntry {
    #[serde(rename = "lastPlayed", default)]
    last_played: Option<String>,
    #[serde(rename = "totalPlayed", default)]
    total_played: Option<u32>,
}

/// Parse a `store/timestamp.json` document. Split from [`load_playtime`]
/// (which owns the filesystem read) so the field mapping is unit-testable
/// against a hand-built JSON string, mirroring `entry_to_game`.
fn parse_timestamp_store(raw: &str) -> HashMap<String, PlayStats> {
    let Ok(entries) = serde_json::from_str::<HashMap<String, TimestampEntry>>(raw) else {
        return HashMap::new();
    };
    entries
        .into_iter()
        .map(|(id, e)| {
            let stats = PlayStats {
                last_played: e.last_played.as_deref().and_then(parse_iso8601_utc),
                playtime_minutes: e.total_played,
            };
            (id, stats)
        })
        .collect()
}

/// Every game's play stats Heroic has recorded, keyed by `app_name` (the same
/// id as [`HeroicGame::app_name`] / `Game::heroic_id`). Any absence — no
/// Heroic, no store file, malformed JSON — yields an empty map rather than an
/// error, matching [`list_sideloaded`]'s tolerance.
pub fn load_playtime() -> HashMap<String, PlayStats> {
    let Some(path) = timestamp_path() else {
        return HashMap::new();
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };
    parse_timestamp_store(&raw)
}

/// Parse a JS `Date.toISOString()`-shaped UTC timestamp
/// (`"2026-08-14T10:28:55.092Z"`, always this exact 24-byte shape — Heroic
/// never writes anything else) into Unix seconds. `None` on any mismatch, so
/// one bad entry is skipped rather than failing the whole store.
fn parse_iso8601_utc(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() != 24
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'.'
        || b[23] != b'Z'
    {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (n(0..4)?, n(5..7)?, n(8..10)?, n(11..13)?, n(14..16)?, n(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let days = days_from_civil(y, mo as u32, d as u32);
    let secs = days.checked_mul(86_400)?.checked_add(h * 3600 + mi * 60 + se)?;
    u64::try_from(secs).ok()
}

/// Civil date (year/month/day) -> days since 1970-01-01. Howard Hinnant's
/// well-known constexpr algorithm — see
/// <http://howardhinnant.github.io/date_algorithms.html#days_from_civil>.
/// Hand-rolled rather than pulling in a date crate: this project has none, and
/// Heroic's timestamp format is fixed enough that this is the whole job.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (i64::from(m) + 9) % 12; // [0, 11]
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
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
    // Outermost first, by the Steam builder's own rank — catalog order put
    // game-performance outside gamescope.
    let mut wrappers = wrappers.to_vec();
    wrappers.sort_by_key(Wrapper::rank);
    for w in &wrappers {
        match w {
            Wrapper::Mangohud => mangohud = true,
            Wrapper::Gamemoderun => gamemode = true,
            // Heroic has no native game-performance toggle, so pass it as a
            // generic prefix wrapper it runs the game through.
            Wrapper::GamePerformance => {
                wrapper_opts.push(json!({ "exe": "game-performance", "args": "" }));
            }
            Wrapper::Plain(p) => {
                wrapper_opts.push(json!({ "exe": bins.program(&p.program), "args": "" }));
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
    if !is_valid_app_name(app_name) {
        return Err(format!("not a Heroic app name: {app_name:?}"));
    }
    let path =
        game_config_path(app_name).ok_or_else(|| "Heroic config directory not found".to_string())?;
    inject_at(&path, app_name, env, wrappers, bins)
}

/// [`inject`] against an explicit config file path — the I/O half, split out
/// so tests can point it at a scratch directory.
fn inject_at(
    path: &std::path::Path,
    app_name: &str,
    env: &[(String, String)],
    wrappers: &[Wrapper],
    bins: &Bins,
) -> Result<InjectResult, String> {
    let path = path.to_path_buf();

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

    crate::fsutil::write_atomic(&path, out.as_bytes())?;

    Ok(InjectResult {
        config_path: path.display().to_string(),
        backup_path: backup.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("protongen-heroic-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn app_names_that_could_leave_games_config_are_rejected() {
        for ok in ["7Hm5qmyaYmaSZ45Mqo3u4s", "1207658924", "amzn1.adg.product.ab-12_c"] {
            assert!(is_valid_app_name(ok), "{ok}");
        }
        for bad in ["", "../x", "a/b", "..", ".hidden", "a b", "x\0"] {
            assert!(!is_valid_app_name(bad), "{bad:?}");
            assert!(inject(bad, &[], &[], &Bins::default()).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn wrapper_options_are_outermost_first_like_the_steam_builder() {
        let bins = Bins { gamescope: "gamescope".into(), ..Bins::default() };
        let wrappers = [Wrapper::GamePerformance, Wrapper::Gamescope("-f".into())];
        let out = apply_to_config(base_config(), "7Hm5", &[], &wrappers, &bins);
        let exes: Vec<&str> = out["7Hm5"]["wrapperOptions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["exe"].as_str().unwrap())
            .collect();
        assert_eq!(exes, ["gamescope", "game-performance"]);
    }

    #[test]
    fn inject_backs_up_then_patches_in_place() {
        let dir = scratch("inject");
        let path = dir.join("7Hm5.json");
        let original = serde_json::to_string_pretty(&base_config()).unwrap();
        std::fs::write(&path, &original).unwrap();

        let env = vec![("DXVK_HDR".to_string(), "1".to_string())];
        let res = inject_at(&path, "7Hm5", &env, &[], &Bins::default()).unwrap();

        assert_eq!(std::fs::read_to_string(&res.backup_path).unwrap(), original);
        let patched: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(patched["7Hm5"]["enviromentOptions"][0]["key"], "DXVK_HDR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inject_refuses_a_missing_or_invalid_config_without_writing() {
        let dir = scratch("refuse");
        let missing = dir.join("7Hm5.json");
        assert!(inject_at(&missing, "7Hm5", &[], &[], &Bins::default()).is_err());
        assert!(!missing.exists(), "must not create a partial config");

        std::fs::write(&missing, "{ not json").unwrap();
        assert!(inject_at(&missing, "7Hm5", &[], &[], &Bins::default()).is_err());
        assert_eq!(std::fs::read_to_string(&missing).unwrap(), "{ not json");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no backup for a refused write");
        let _ = std::fs::remove_dir_all(&dir);
    }

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
        let games = parse_library(raw, "sideload", false);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].app_name, "abc");
        assert_eq!(games[0].title, "Crimson Desert");
        assert_eq!(games[0].executable.as_deref(), Some("/games/cd/CrimsonDesert.exe"));
        // art_cover wins over art_square when both are present.
        assert_eq!(games[0].art.as_deref(), Some("https://cdn2.steamgriddb.com/grid/abc.png"));
    }

    #[test]
    fn parse_library_reads_the_library_key_alias_and_joins_install_path() {
        // legendary/nile's store_cache file uses "library", not "games", and
        // stores just the exe filename under install_path.
        let raw = r#"{
            "library": [
                { "runner": "legendary", "app_name": "Snowdrop", "title": "Jackbox Party Pack 4",
                  "install": { "executable": "Jackbox.exe",
                               "install_path": "/home/u/Games/Heroic/JackboxPartyPack4",
                               "is_dlc": false },
                  "is_installed": true }
            ]
        }"#;
        let games = parse_library(raw, "legendary", true);
        assert_eq!(games.len(), 1);
        assert_eq!(
            games[0].executable.as_deref(),
            Some("/home/u/Games/Heroic/JackboxPartyPack4/Jackbox.exe")
        );
    }

    #[test]
    fn parse_library_drops_dlc_and_not_installed_when_required() {
        let raw = r#"{
            "games": [
                { "runner": "gog", "app_name": "base", "title": "Base Game",
                  "install": { "is_dlc": false }, "is_installed": true },
                { "runner": "gog", "app_name": "dlc", "title": "Some DLC",
                  "install": { "is_dlc": true }, "is_installed": true },
                { "runner": "gog", "app_name": "owned", "title": "Owned Not Installed",
                  "install": { "is_dlc": false }, "is_installed": false }
            ]
        }"#;
        let games = parse_library(raw, "gog", true);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].app_name, "base");
    }

    #[test]
    fn parse_library_tolerates_malformed_or_missing_data() {
        assert!(parse_library("not json", "legendary", true).is_empty());
        assert!(parse_library("{}", "legendary", true).is_empty());
    }

    #[test]
    fn resolve_executable_prefers_an_already_absolute_path() {
        assert_eq!(
            resolve_executable(Some("/should/be/ignored"), Some("/games/x/game.exe")),
            Some("/games/x/game.exe".to_string())
        );
    }

    #[test]
    fn resolve_executable_joins_relative_exe_with_install_path() {
        assert_eq!(
            resolve_executable(Some("/games/x/"), Some("game.exe")),
            Some("/games/x/game.exe".to_string())
        );
        assert_eq!(
            resolve_executable(Some("/games/x"), Some("game.exe")),
            Some("/games/x/game.exe".to_string())
        );
    }

    #[test]
    fn resolve_executable_handles_absent_fields() {
        assert_eq!(resolve_executable(None, Some("game.exe")), Some("game.exe".to_string()));
        assert_eq!(resolve_executable(Some("/games/x"), None), None);
        assert_eq!(resolve_executable(Some("/games/x"), Some("")), None);
    }

    #[test]
    fn entry_to_game_prefers_art_cover_falls_back_to_art_square() {
        let mut e = LibraryEntry {
            runner: "sideload".to_string(),
            app_name: "a".to_string(),
            title: "Has cover".to_string(),
            install: LibraryInstall::default(),
            is_installed: true,
            art_cover: Some("file:///covers/a.jpg".to_string()),
            art_square: Some("file:///covers/a-sq.jpg".to_string()),
        };
        assert_eq!(entry_to_game(e).art.as_deref(), Some("file:///covers/a.jpg"));

        e = LibraryEntry {
            app_name: "b".to_string(),
            art_cover: None,
            art_square: Some("file:///covers/b-sq.jpg".to_string()),
            ..blank_entry()
        };
        assert_eq!(entry_to_game(e).art.as_deref(), Some("file:///covers/b-sq.jpg"));

        e = LibraryEntry { app_name: "c".to_string(), ..blank_entry() };
        assert_eq!(entry_to_game(e).art, None);
    }

    fn blank_entry() -> LibraryEntry {
        LibraryEntry {
            runner: "sideload".to_string(),
            app_name: String::new(),
            title: String::new(),
            install: LibraryInstall::default(),
            is_installed: false,
            art_cover: None,
            art_square: None,
        }
    }

    #[test]
    fn parse_iso8601_utc_epoch_is_zero() {
        assert_eq!(parse_iso8601_utc("1970-01-01T00:00:00.000Z"), Some(0));
    }

    #[test]
    fn parse_iso8601_utc_matches_date_command() {
        // Cross-checked: `date -u -d '2026-08-15T18:30:49Z' +%s` -> 1786818649
        assert_eq!(
            parse_iso8601_utc("2026-08-15T18:30:49.540Z"),
            Some(1_786_818_649)
        );
    }

    #[test]
    fn parse_iso8601_utc_handles_leap_day() {
        // `date -u -d '2024-02-29T00:00:00Z' +%s` -> 1709164800
        assert_eq!(
            parse_iso8601_utc("2024-02-29T00:00:00.000Z"),
            Some(1_709_164_800)
        );
    }

    #[test]
    fn parse_iso8601_utc_rejects_malformed_input() {
        assert_eq!(parse_iso8601_utc(""), None);
        assert_eq!(parse_iso8601_utc("2026-08-15"), None);
        assert_eq!(parse_iso8601_utc("2026-13-01T00:00:00.000Z"), None);
        assert_eq!(parse_iso8601_utc("2026-08-15T18:30:49.540"), None); // no trailing Z
    }

    #[test]
    fn parse_timestamp_store_reads_a_realistic_fixture() {
        let raw = r#"{
            "4q6N4i6zTdYA6RjzHj7hto": {
                "firstPlayed": "2026-08-15T18:30:33.445Z",
                "lastPlayed": "2026-08-15T18:30:49.540Z",
                "totalPlayed": 0
            },
            "f-s6hmOsYkhbS-bOICLR2k": {
                "firstPlayed": "2026-08-18T21:57:44.422Z",
                "lastPlayed": "2026-09-04T14:48:01.412Z",
                "totalPlayed": 514
            }
        }"#;
        let map = parse_timestamp_store(raw);
        assert_eq!(map.len(), 2);
        // totalPlayed: 0 is a real value, not "absent" — must survive as Some(0).
        let a = map.get("4q6N4i6zTdYA6RjzHj7hto").unwrap();
        assert_eq!(a.playtime_minutes, Some(0));
        assert_eq!(a.last_played, Some(1_786_818_649));
        let b = map.get("f-s6hmOsYkhbS-bOICLR2k").unwrap();
        assert_eq!(b.playtime_minutes, Some(514));
        assert!(b.last_played.is_some());
    }

    #[test]
    fn parse_timestamp_store_keeps_playtime_when_last_played_is_bad() {
        let raw = r#"{ "abc": { "lastPlayed": "not-a-date", "totalPlayed": 12 } }"#;
        let map = parse_timestamp_store(raw);
        let a = map.get("abc").unwrap();
        assert_eq!(a.last_played, None);
        assert_eq!(a.playtime_minutes, Some(12));
    }

    #[test]
    fn parse_timestamp_store_returns_empty_map_on_garbage() {
        assert!(parse_timestamp_store("not json").is_empty());
        assert!(parse_timestamp_store("").is_empty());
    }
}
