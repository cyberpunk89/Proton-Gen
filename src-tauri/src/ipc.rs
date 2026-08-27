//! Tauri command surface: a thin, serializable bridge over the pure logic
//! modules. Frontend selection state is the existing `store::Config`; the
//! catalog / recipes / runtimes / games / hardware are sent once via `bootstrap`.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use serde::Serialize;
use steamlocate::SteamDir;
use tauri::State;

use crate::art;
use crate::builder::{self, Wrapper};
use crate::compose;
use crate::diff::{self, LaunchDiff};
use crate::explain::{self, Token};
use crate::games::{self, GameSource};
use crate::hardware::{self, Hardware};
use crate::heroic;
use crate::lint;
use crate::llm::{self, LlmRequest, LlmSuggestion, RecipeRef, TroubleshootRequest, TroubleshootResult};
use crate::optiscaler_upgrade;
use crate::params::{Catalog, ConfigWarning};
use crate::parser;
use crate::protondb::{self, Tier};
use crate::recipes::{self, Recipe, Recipes};
use crate::runtime::{self, RuntimeKind};
use crate::steam;
use crate::steamcfg;
use crate::store::{self, Config, Store};
use crate::update::{self, UpdateInfo};

/// A runtime, flattened for the frontend (path + kind as strings).
#[derive(Clone, Serialize)]
pub struct RuntimeDto {
    pub internal_name: String,
    pub display_name: String,
    pub kind: String,
    pub path: String,
}

/// A game/shortcut, flattened for the frontend.
///
/// `last_played` / `playtime_minutes` come from `localconfig.vdf`, not from the
/// app manifest — `steamlocate::App` has no such fields. Both are `None` for a
/// game Steam has never recorded, and always `None` for non-Steam shortcuts
/// (Steam keeps no per-app user record for them).
#[derive(Clone, Serialize)]
pub struct GameDto {
    pub app_id: u32,
    pub name: String,
    pub source: String,
    pub executable: Option<String>,
    pub installed: bool,
    /// Unix seconds.
    pub last_played: Option<u64>,
    pub playtime_minutes: Option<u32>,
    /// Heroic's per-game id — `Some` only for `source == "heroic"`; the key the
    /// `inject_heroic` command needs to locate the game's config file.
    pub heroic_id: Option<String>,
    /// Absolute install directory, when resolvable — see `games::Game::install_dir`.
    /// Used by the OptiScaler-upgrade commands to find/write files there.
    pub install_dir: Option<String>,
}

/// One game's Proton log, read for the diagnostics viewer.
///
/// `PROTON_LOG=1` writes `~/steam-<appid>.log`. This is a read-only view of that
/// file; a missing file is a normal `present: false` result, not an error, so
/// the viewer can say "no log yet — enable logging and relaunch" rather than
/// showing a failure.
#[derive(Clone, Serialize)]
pub struct ProtonLog {
    /// Whether the log file exists.
    pub present: bool,
    /// The path we looked at, shown even when absent so the user knows where the
    /// log will appear.
    pub path: String,
    /// The tail of the log (last [`LOG_TAIL_BYTES`]), or empty when absent.
    pub tail: String,
    /// Total size in bytes.
    pub size: u64,
    /// True when the file was larger than the tail we returned (the head was cut).
    pub truncated: bool,
    /// Lines from the tail matching common error/warning markers, surfaced first
    /// so the likely-relevant bits are one glance away.
    pub error_lines: Vec<String>,
}

/// The "catalog refreshed for an older build" banner data.
#[derive(Clone, Serialize)]
pub struct StaleInfo {
    pub installed: String,
    pub catalog: String,
    pub updated: String,
}

/// Everything the frontend needs at startup, in one round-trip.
#[derive(Clone, Serialize)]
pub struct Bootstrap {
    pub steam_root: Option<String>,
    pub load_error: Option<String>,
    pub catalog: Catalog,
    pub categories: Vec<String>,
    pub recipes: Vec<Recipe>,
    pub runtimes: Vec<RuntimeDto>,
    pub games: Vec<GameDto>,
    pub hardware: Hardware,
    pub store: Store,
    /// appid (string) -> current Steam launch options.
    pub launch_options: HashMap<String, String>,
    /// appid (string) -> currently mapped compat tool internal name.
    pub compat_tools: HashMap<String, String>,
    /// required binary name -> whether it's on $PATH (drives installed/missing badges).
    pub requires_status: HashMap<String, bool>,
    pub stale: Option<StaleInfo>,
    /// User config overrides that failed to parse and were ignored.
    pub config_warnings: Vec<ConfigWarning>,
}

/// Shared application state: immutable discovery results + a mutable store.
pub struct AppState {
    catalog: Catalog,
    recipes: Recipes,
    hardware: Hardware,
    steam_root: Option<String>,
    load_error: Option<String>,
    runtimes: Vec<RuntimeDto>,
    games: Vec<GameDto>,
    launch_options: HashMap<String, String>,
    compat_tools: HashMap<String, String>,
    requires_status: HashMap<String, bool>,
    stale: Option<StaleInfo>,
    /// Static warnings from the TOML overrides. Path warnings live on
    /// `Discovery` instead, because a rescan can clear them.
    config_warnings: Vec<ConfigWarning>,
    path_warnings: Mutex<Vec<ConfigWarning>>,
    store: Mutex<Store>,
}

impl AppState {
    /// Wrapper program names, with the user's Settings overrides applied.
    ///
    /// Built per call rather than cached: `save_store` can replace the whole
    /// store mid-session, and a cached copy would stay stale until restart.
    fn bins(&self) -> builder::Bins {
        builder::Bins::with_overrides(&self.store.lock().unwrap().paths.bins)
    }

    /// Find a discovered game by appid — the OptiScaler-upgrade commands look
    /// up `install_dir` this way rather than trusting a path the frontend
    /// sends, so the write target always comes from the same discovery pass
    /// `bootstrap`/`rescan` already validated.
    fn game(&self, app_id: u32) -> Option<&GameDto> {
        self.games.iter().find(|g| g.app_id == app_id)
    }
}

/// Results of a filesystem re-scan: everything that can change while the app is
/// running (a game installed, a Proton runtime added). Recomputed by both
/// `AppState::new()` and the `rescan` command.
struct Discovery {
    steam_root: Option<String>,
    load_error: Option<String>,
    runtimes: Vec<RuntimeDto>,
    games: Vec<GameDto>,
    launch_options: HashMap<String, String>,
    compat_tools: HashMap<String, String>,
    stale: Option<StaleInfo>,
    /// Configured paths discovery could not use. Recomputed every scan, so
    /// fixing a path clears its banner.
    path_warnings: Vec<ConfigWarning>,
}

/// Re-run Steam / runtime / games discovery. Pure and idempotent — safe to call
/// repeatedly. `catalog` is only read (for staleness); it never changes here.
/// `paths` carries the user's Settings overrides; no discovery module reads the
/// store itself.
fn scan_discovery(catalog: &Catalog, paths: &store::Paths) -> Discovery {
    let mut steam_root = None;
    let mut load_error = None;
    let mut runtimes_raw = Vec::new();
    let mut games = Vec::new();
    let mut launch_options = HashMap::new();
    let mut compat_tools = HashMap::new();
    let mut path_warnings = Vec::new();

    match steam::locate_native(&paths.steam_roots, &mut path_warnings) {
        Ok(dir) => {
            steam_root = Some(steam::root_display(&dir));
            runtimes_raw = runtime::discover(&dir, &paths.proton_dirs, &mut path_warnings);
            // localconfig first: `list_games_dto` reads last-played/playtime out
            // of it, so the parsed map has to exist before the games are built.
            let app_cfgs = steamcfg::current_app_cfgs(&dir);
            games = list_games_dto(&dir, &app_cfgs, &paths.steam_libraries, &mut path_warnings);
            launch_options = stringify_keys(steamcfg::launch_options(&app_cfgs));
            compat_tools = stringify_keys(steamcfg::current_compat_tools(&dir));
        }
        Err(e) => {
            load_error = Some(e.to_string());
            // Heroic games don't need Steam. With no Steam install, `list_games`
            // never runs, so surface sideloaded Heroic games on their own here.
            games = games::dedup_and_sort(games::list_heroic_games())
                .into_iter()
                .map(|g| game_dto(g, &HashMap::new()))
                .collect();
        }
    }

    let stale = compute_stale(catalog, &runtimes_raw);
    let runtimes = runtimes_raw.iter().map(runtime_dto).collect();

    Discovery {
        steam_root,
        load_error,
        runtimes,
        games,
        launch_options,
        compat_tools,
        stale,
        path_warnings,
    }
}

impl AppState {
    pub fn new() -> Self {
        let (catalog, catalog_warning) = Catalog::load();
        let (recipes, recipes_warning) = Recipes::load();
        let config_warnings = catalog_warning.into_iter().chain(recipes_warning).collect();
        let hardware = hardware::detect();
        let store = Store::load();

        let d = scan_discovery(&catalog, &store.paths);
        let requires_status =
            compute_requires_status(&catalog, &builder::Bins::with_overrides(&store.paths.bins));

        Self {
            catalog,
            recipes,
            hardware,
            steam_root: d.steam_root,
            load_error: d.load_error,
            runtimes: d.runtimes,
            games: d.games,
            launch_options: d.launch_options,
            compat_tools: d.compat_tools,
            requires_status,
            stale: d.stale,
            config_warnings,
            path_warnings: Mutex::new(d.path_warnings),
            store: Mutex::new(store),
        }
    }
}

fn runtime_dto(r: &runtime::Runtime) -> RuntimeDto {
    let kind = match r.kind {
        RuntimeKind::System => "system",
        RuntimeKind::User => "user",
        RuntimeKind::Bundled => "valve",
        RuntimeKind::Custom => "custom",
    };
    RuntimeDto {
        internal_name: r.internal_name.clone(),
        display_name: r.display_name.clone(),
        kind: kind.to_string(),
        path: r.path.display().to_string(),
    }
}

/// Map one discovery [`games::Game`] to its serialized DTO. `app_cfgs` supplies
/// last-played/playtime, which only Steam apps have — a shortcut's or Heroic
/// game's synthetic appid is a hash that indexes nothing in `localconfig.vdf`.
fn game_dto(g: games::Game, app_cfgs: &HashMap<u32, steamcfg::AppUserCfg>) -> GameDto {
    let cfg = match g.source {
        GameSource::Steam => app_cfgs.get(&g.app_id),
        GameSource::NonSteam | GameSource::Heroic => None,
    };
    GameDto {
        app_id: g.app_id,
        name: g.name,
        source: g.source.label().to_string(),
        executable: g.executable,
        installed: g.installed,
        last_played: cfg.and_then(|c| c.last_played),
        playtime_minutes: cfg.and_then(|c| c.playtime_minutes),
        heroic_id: g.heroic_id,
        install_dir: g.install_dir.map(|p| p.display().to_string()),
    }
}

fn list_games_dto(
    dir: &SteamDir,
    app_cfgs: &HashMap<u32, steamcfg::AppUserCfg>,
    extra_libraries: &[String],
    warn: &mut Vec<ConfigWarning>,
) -> Vec<GameDto> {
    games::list_games(dir, extra_libraries, warn)
        .into_iter()
        .map(|g| game_dto(g, app_cfgs))
        .collect()
}

fn stringify_keys(m: HashMap<u32, String>) -> HashMap<String, String> {
    m.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// For every distinct `requires` binary in the catalog, whether it's on $PATH.
///
/// Seeded from `Bins` first, so the badge reflects the token the builder will
/// actually emit rather than the default name. That also gives `umu-run` a badge
/// at all: it is neither a `[[wrapper]]` nor an `[[env]]`, so there is no TOML
/// entry to hang `requires` on, yet an entire mode depends on it.
fn compute_requires_status(catalog: &Catalog, bins: &builder::Bins) -> HashMap<String, bool> {
    let mut out = HashMap::new();
    for (name, program) in bins.pairs() {
        out.insert(name.to_string(), crate::which::is_installed(program));
    }
    let extra = catalog
        .wrappers
        .iter()
        .filter_map(|w| w.requires.clone())
        .chain(catalog.envs.iter().filter_map(|e| e.requires.clone()));
    for bin in extra {
        out.entry(bin)
            .or_insert_with_key(|b| crate::which::is_installed(b));
    }
    out
}

fn compute_stale(catalog: &Catalog, runtimes: &[runtime::Runtime]) -> Option<StaleInfo> {
    let cat_build = catalog.meta.proton_cachyos_build.as_deref()?;
    let installed = runtime::installed_cachyos_build(runtimes)?;
    if installed.as_str() > cat_build {
        Some(StaleInfo {
            installed,
            catalog: cat_build.to_string(),
            updated: catalog.meta.updated.clone().unwrap_or_else(|| "?".to_string()),
        })
    } else {
        None
    }
}

// ----------------------------- commands -----------------------------

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> Bootstrap {
    let store = state.store.lock().unwrap().clone();
    Bootstrap {
        steam_root: state.steam_root.clone(),
        load_error: state.load_error.clone(),
        catalog: state.catalog.clone(),
        categories: state.catalog.categories(),
        recipes: state.recipes.recipes.clone(),
        runtimes: state.runtimes.clone(),
        games: state.games.clone(),
        hardware: state.hardware.clone(),
        store,
        launch_options: state.launch_options.clone(),
        compat_tools: state.compat_tools.clone(),
        requires_status: state.requires_status.clone(),
        stale: state.stale.clone(),
        // Parse warnings plus whatever the startup scan made of the configured
        // paths, so a bad path is visible from the first frame.
        config_warnings: state
            .config_warnings
            .iter()
            .cloned()
            .chain(state.path_warnings.lock().unwrap().iter().cloned())
            .collect(),
    }
}

/// Re-scan Steam / runtimes / games and return a fresh `Bootstrap` so the UI can
/// pick up newly-installed games or Proton runtimes without a restart. The
/// static fields (catalog, recipes, hardware) and the current store are reused
/// unchanged. `requires_status` and the path warnings are *not* static — a
/// rescan is how a corrected Settings path clears its banner and turns a binary
/// override's badge green. `AppState`'s discovery snapshot is intentionally
/// left untouched — no command reads it after startup, so there's nothing to keep
/// in sync (build_command/lint work off the passed Config + catalog).
#[tauri::command]
pub fn rescan(state: State<'_, AppState>) -> Bootstrap {
    let store = state.store.lock().unwrap().clone();
    let d = scan_discovery(&state.catalog, &store.paths);
    let requires_status =
        compute_requires_status(&state.catalog, &builder::Bins::with_overrides(&store.paths.bins));
    let warnings = state
        .config_warnings
        .iter()
        .cloned()
        .chain(d.path_warnings.iter().cloned())
        .collect();
    *state.path_warnings.lock().unwrap() = d.path_warnings;
    Bootstrap {
        steam_root: d.steam_root,
        load_error: d.load_error,
        catalog: state.catalog.clone(),
        categories: state.catalog.categories(),
        recipes: state.recipes.recipes.clone(),
        runtimes: d.runtimes,
        games: d.games,
        hardware: state.hardware.clone(),
        store,
        launch_options: d.launch_options,
        compat_tools: d.compat_tools,
        requires_status,
        stale: d.stale,
        config_warnings: warnings,
    }
}

/// Assemble the launch command for the given config. `proton_path` is the
/// selected runtime's install dir (used as PROTONPATH in umu mode).
#[tauri::command]
pub fn build_command(
    state: State<'_, AppState>,
    config: Config,
    proton_path: Option<String>,
) -> String {
    // Built per call rather than cached on AppState: `save_store` can replace
    // the store mid-session, and a cached copy would stay stale until restart.
    // Cheap — this command is already debounced ~60 ms on the frontend.
    let bins = state.bins();
    compose::assemble(&state.catalog, &config, proton_path.as_deref(), &bins)
}

/// Write the current `config`'s env vars + wrappers into a Heroic sideloaded
/// game's per-game config (`GamesConfig/<app_name>.json`). The one sanctioned
/// write outside protongen's own state: it backs up first, preserves every key
/// it doesn't own, and writes atomically. `app_name` is the game's `heroic_id`.
///
/// Reuses the same resolver as the preview, so what lands in Heroic equals what
/// the command box shows (minus the umu lead vars, which Heroic owns).
#[tauri::command]
pub fn inject_heroic(
    state: State<'_, AppState>,
    app_name: String,
    config: Config,
) -> Result<heroic::InjectResult, String> {
    let (env, wrappers) = compose::resolve_env_wrappers(&state.catalog, &config);
    heroic::inject(&app_name, &env, &wrappers, &state.bins())
}

/// Parse a pasted Steam/umu command into a `Config` (unknown env → extra_env).
#[tauri::command]
pub fn parse_command(state: State<'_, AppState>, input: String) -> Config {
    let p = parser::parse(&input);
    let catalog = &state.catalog;

    // Reconstruct wrapper key/value list from the parsed wrappers.
    let wrappers: Vec<(String, String)> = p
        .wrappers()
        .iter()
        .map(|w| match w {
            Wrapper::Gamescope(a) => ("gamescope".to_string(), a.clone()),
            Wrapper::GamePerformance => ("game-performance".to_string(), String::new()),
            Wrapper::Gamemoderun => ("gamemoderun".to_string(), String::new()),
            Wrapper::Mangohud => ("mangohud".to_string(), String::new()),
        })
        .collect();

    // Enable catalog-known env/wrappers; capture them back in catalog order.
    // Anything the catalog doesn't know comes back as a leftover and goes to the
    // custom-env field, which is what makes the import lossless.
    let (options, unknown) = store::options_from_lists(catalog, &p.env, &wrappers);
    let (env, wrappers) = store::options_to_lists(catalog, &options);

    Config {
        umu: p.umu,
        runtime: None,
        env,
        wrappers,
        extra_env: compose::format_extra_env(&unknown),
        umu_exe: p.umu_exe,
        umu_wineprefix: p.umu_wineprefix.unwrap_or_default(),
        umu_gameid: p.umu_gameid.unwrap_or_default(),
        game_args: p.game_args,
    }
}

/// Tokenize a launch command for the annotated preview. Stateless: tokens carry
/// only a catalog `key`, which the frontend resolves against the already-loaded
/// catalog.
#[tauri::command]
pub fn explain_command(command: String) -> Vec<Token> {
    explain::explain(&command)
}

/// Compare a built launch command against the one Steam currently has set.
/// Stateless and pure — see `diff.rs` for what is deliberately normalised away.
///
/// The *contextual* states (no game selected, non-Steam shortcut, generic
/// command) stay out of the DTO: the frontend already holds `selectedAppId`,
/// `game.source` and `app.umu`, and pushing them here would drag game/mode
/// state through an otherwise trivially pure function.
#[tauri::command]
pub fn launch_diff(built: String, current: String) -> LaunchDiff {
    diff::compare(&built, &current)
}

/// Applied / drifted / not-applied for every remembered game in one call, so
/// the library grid can badge them all at a glance.
///
/// `launch_options` comes from the frontend rather than `AppState`: the
/// discovery snapshot there is deliberately not refreshed by `rescan` (see
/// above), so reading it would silently go stale after a library refresh. The
/// frontend already holds the fresh copy as `app.launchOptions`, and passing it
/// in keeps `diff::statuses` a pure function of its arguments.
#[tauri::command]
pub fn launch_statuses(
    state: State<'_, AppState>,
    memory: BTreeMap<String, Config>,
    launch_options: HashMap<String, String>,
) -> HashMap<String, diff::DiffStatus> {
    diff::statuses(&state.catalog, &memory, &launch_options, &state.bins())
}

/// Merge recipe `index` onto `config`, returning the updated config.
#[tauri::command]
pub fn apply_recipe(state: State<'_, AppState>, index: usize, config: Config) -> Config {
    let catalog = &state.catalog;
    let Some(recipe) = state.recipes.recipes.get(index) else {
        return config;
    };

    let (mut options, leftover) = compose::options_from_config(catalog, &config);
    // Recover keys the catalog no longer knows *before* the recipe merges, so the
    // round-trip through `options_to_lists` below can't erase them (#62). Without
    // this, applying any recipe — even one touching nothing related — silently
    // deleted stale env from the saved config for good.
    let mut extra_env = compose::merge_into_extra_env(&config.extra_env, &leftover);
    recipes::apply(recipe, catalog, &mut options, &mut extra_env);

    let (env, wrappers) = store::options_to_lists(catalog, &options);
    Config {
        env,
        wrappers,
        extra_env,
        ..config
    }
}

/// What applying recipe `index` to `config` would change, without changing it.
#[tauri::command]
pub fn preview_recipe(
    state: State<'_, AppState>,
    index: usize,
    config: Config,
) -> Vec<recipes::RecipeChange> {
    let catalog = &state.catalog;
    let Some(recipe) = state.recipes.recipes.get(index) else {
        return Vec::new();
    };
    let (options, leftover) = compose::options_from_config(catalog, &config);
    // Diff against the same extra_env `apply_recipe` will build, or the preview
    // misreports a stale key the recipe also sets as a fresh addition.
    let extra_env = compose::merge_into_extra_env(&config.extra_env, &leftover);
    recipes::diff(recipe, catalog, &options, &extra_env)
}

/// Conflict / footgun notices for the current config.
#[tauri::command]
pub fn lint(state: State<'_, AppState>, config: Config) -> Vec<lint::Notice> {
    // Leftovers are discarded here on purpose: a rule is written against catalog
    // keys, so a key with no catalog entry has no rule that could name it.
    let (options, _) = compose::options_from_config(&state.catalog, &config);
    // The declared AMD generation is a store field, not a detected one, so it
    // has to be read here rather than off `state.hardware`.
    let gpu_gen = state.store.lock().unwrap().gpu_gen.clone();
    lint::warnings(&state.catalog, &options, &state.hardware, &gpu_gen)
}

/// The ProtonDB community page URL for a Steam app id.
#[tauri::command]
pub fn protondb_url(appid: u32) -> String {
    protondb::page_url(appid)
}

/// Fetch a game's ProtonDB tier summary (off the UI thread).
#[tauri::command]
pub async fn protondb_fetch(appid: u32) -> Result<Tier, String> {
    tauri::async_runtime::spawn_blocking(move || protondb::fetch_blocking(appid))
        .await
        .map_err(|e| e.to_string())?
}

/// Resolve a game's artwork to a `data:` URL (local cache → optional CDN), or
/// `null` if none is available. Runs off the UI thread.
#[tauri::command]
pub async fn game_art(
    state: State<'_, AppState>,
    app_id: u32,
    source: String,
    kind: String,
    online: bool,
) -> Result<Option<String>, String> {
    let steam_root = state.steam_root.clone();
    tauri::async_runtime::spawn_blocking(move || {
        art::fetch(steam_root, app_id, &source, &kind, online)
    })
    .await
    .map_err(|e| e.to_string())
}

/// Bytes of log tail to return. Proton logs can reach hundreds of MB over a long
/// session; the viewer only ever needs the end, and the head is stale by then.
const LOG_TAIL_BYTES: u64 = 64 * 1024;
/// Cap on surfaced error lines, so a log that is nothing but warnings can't
/// balloon the payload the frontend has to render.
const LOG_ERROR_LINES: usize = 200;

/// The default Proton log path for an app id — `$HOME/steam-<appid>.log`, where
/// `PROTON_LOG=1` writes with no `PROTON_LOG_DIR`. `None` only when `$HOME` is
/// unset, which on a desktop session does not happen.
fn proton_log_path(app_id: u32) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(std::path::PathBuf::from(home).join(format!("steam-{app_id}.log")))
}

/// True when a log line looks like something worth reading first. Case-folded
/// substring match on a small marker set — deliberately conservative so the
/// "problems" list stays short enough to scan.
fn is_error_line(line: &str) -> bool {
    const MARKERS: [&str; 8] =
        ["err", "fail", "crash", "fixme", "abort", "assert", "unsupported", "not found"];
    let lower = line.to_ascii_lowercase();
    MARKERS.iter().any(|m| lower.contains(m))
}

/// Read the tail of `app_id`'s Proton log. Pure filesystem work, split out so the
/// command is just the `spawn_blocking` wrapper.
fn read_proton_log_blocking(app_id: u32) -> ProtonLog {
    use std::io::{Read, Seek, SeekFrom};

    let absent = |path: String| ProtonLog {
        present: false,
        path,
        tail: String::new(),
        size: 0,
        truncated: false,
        error_lines: Vec::new(),
    };

    let Some(path) = proton_log_path(app_id) else {
        return absent("$HOME/steam-<appid>.log".to_string());
    };
    let path_str = path.display().to_string();

    let Ok(meta) = std::fs::metadata(&path) else {
        return absent(path_str);
    };
    let size = meta.len();
    let truncated = size > LOG_TAIL_BYTES;

    let mut file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return absent(path_str),
    };
    if truncated {
        // Seek to the last window; a failed seek just means we read from 0.
        let _ = file.seek(SeekFrom::Start(size - LOG_TAIL_BYTES));
    }
    let mut buf = Vec::new();
    if file.read_to_end(&mut buf).is_err() {
        return absent(path_str);
    }

    let mut tail = String::from_utf8_lossy(&buf).into_owned();
    // When we seeked mid-file we almost certainly landed inside a line; drop that
    // partial fragment so the first shown line is whole.
    if truncated {
        if let Some(nl) = tail.find('\n') {
            tail = tail[nl + 1..].to_string();
        }
    }

    let error_lines = tail
        .lines()
        .filter(|l| is_error_line(l))
        .take(LOG_ERROR_LINES)
        .map(|l| l.trim().to_string())
        .collect();

    ProtonLog {
        present: true,
        path: path_str,
        tail,
        size,
        truncated,
        error_lines,
    }
}

#[cfg(test)]
mod log_tests {
    use super::*;

    #[test]
    fn error_lines_match_the_common_markers_case_insensitively() {
        assert!(is_error_line("wine: FIXME:module stub"));
        assert!(is_error_line("err:  vulkan device lost"));
        assert!(is_error_line("Assertion failed"));
        assert!(is_error_line("file not found"));
        assert!(!is_error_line("info: loaded 42 shaders"));
        assert!(!is_error_line("frame time 16ms"));
    }
}

/// Read the Proton log for `app_id` (`PROTON_LOG=1` writes `~/steam-<appid>.log`).
/// Read-only and off the UI thread; a missing file is a normal `present: false`
/// result, not an error.
#[tauri::command]
pub async fn read_proton_log(app_id: u32) -> Result<ProtonLog, String> {
    tauri::async_runtime::spawn_blocking(move || read_proton_log_blocking(app_id))
        .await
        .map_err(|e| e.to_string())
}

/// Analyze a game's Proton log with the configured local LLM (off the UI thread).
/// Opt-in: the endpoint/model come from the store, while the catalog allow-list
/// and detected-hardware summary are added here from `AppState`. Read-only — the
/// result is advice; the frontend applies a change only when the user clicks.
#[tauri::command]
pub async fn llm_analyze(
    state: State<'_, AppState>,
    req: LlmRequest,
) -> Result<LlmSuggestion, String> {
    let (endpoint, model) = {
        let s = state.store.lock().unwrap();
        (s.llm_endpoint.clone(), s.llm_model.clone())
    };
    let hardware = state.hardware.llm_context();
    let mut catalog_keys: Vec<String> =
        state.catalog.envs.iter().map(|e| e.key.clone()).collect();
    catalog_keys.extend(state.catalog.wrappers.iter().map(|w| w.key.clone()));
    tauri::async_runtime::spawn_blocking(move || {
        llm::suggest_blocking(req, &endpoint, &model, &hardware, &catalog_keys)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Diagnose a free-text symptom with the local LLM (off the UI thread),
/// recommending existing Fix recipes (by IPC index) where they fit and proposing
/// catalog changes otherwise. Opt-in; endpoint/model from the store, the Fix
/// recipe list + hardware summary + catalog allow-list from `AppState`.
#[tauri::command]
pub async fn llm_troubleshoot(
    state: State<'_, AppState>,
    req: TroubleshootRequest,
) -> Result<TroubleshootResult, String> {
    let (endpoint, model) = {
        let s = state.store.lock().unwrap();
        (s.llm_endpoint.clone(), s.llm_model.clone())
    };
    let hardware = state.hardware.llm_context();
    // Only Fix recipes are offered, tagged with their stable IPC index (position
    // in the full recipe list, which `apply_recipe` indexes by).
    let recipes: Vec<RecipeRef> = state
        .recipes
        .recipes
        .iter()
        .enumerate()
        .filter(|(_, r)| r.kind == recipes::RecipeKind::Fix)
        .map(|(i, r)| RecipeRef {
            index: i as u32,
            name: r.name.clone(),
            symptom: r.symptom.clone().unwrap_or_default(),
            description: r.description.clone(),
        })
        .collect();
    let mut catalog_keys: Vec<String> =
        state.catalog.envs.iter().map(|e| e.key.clone()).collect();
    catalog_keys.extend(state.catalog.wrappers.iter().map(|w| w.key.clone()));
    tauri::async_runtime::spawn_blocking(move || {
        llm::troubleshoot_blocking(req, &recipes, &endpoint, &model, &hardware, &catalog_keys)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// List the models the configured local LLM endpoint is serving (off the UI
/// thread), for the Settings model picker / connection test.
#[tauri::command]
pub async fn llm_models(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let endpoint = state.store.lock().unwrap().llm_endpoint.clone();
    tauri::async_runtime::spawn_blocking(move || llm::list_models_blocking(&endpoint))
        .await
        .map_err(|e| e.to_string())?
}

/// Replace and persist the whole store (theme, presets, per-game memory, dismissals).
#[tauri::command]
pub fn save_store(state: State<'_, AppState>, store: Store) -> Result<(), String> {
    let mut guard = state.store.lock().unwrap();
    *guard = store;
    guard.save()
}

/// Whether `app_id` already has an OptiScaler install to refresh — cheap
/// filesystem check, no network. `install_dir` comes from `AppState`'s own
/// discovery, never from the caller (see `AppState::game`).
#[tauri::command]
pub fn optiscaler_status(state: State<'_, AppState>, app_id: u32) -> optiscaler_upgrade::OptiscalerStatus {
    let dir = state.game(app_id).and_then(|g| g.install_dir.as_deref()).map(std::path::Path::new);
    optiscaler_upgrade::detect(dir)
}

/// Check the latest upstream OptiScaler release (off the UI thread). Global —
/// not per-game — so the frontend can show "latest: vX.Y.Z" without a game
/// selected. Errors surface directly to the caller; there's no banner to keep
/// quiet for, unlike `check_for_update`.
#[tauri::command]
pub async fn optiscaler_latest() -> Result<optiscaler_upgrade::OptiscalerRelease, String> {
    tauri::async_runtime::spawn_blocking(optiscaler_upgrade::check_latest)
        .await
        .map_err(|e| e.to_string())?
}

/// Download the latest OptiScaler release and extract it into `app_id`'s
/// install directory (off the UI thread). The one command in this file that
/// writes into a game's own folder — see `optiscaler_upgrade`'s doc comment.
#[tauri::command]
pub async fn optiscaler_fetch(
    state: State<'_, AppState>,
    app_id: u32,
) -> Result<optiscaler_upgrade::OptiscalerExtractResult, String> {
    let dir = state
        .game(app_id)
        .and_then(|g| g.install_dir.clone())
        .ok_or_else(|| "no install directory known for this game".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        optiscaler_upgrade::fetch_and_extract(std::path::Path::new(&dir))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Check GitHub Releases for a newer version (off the UI thread). A failed check
/// returns an error the frontend swallows — it must never block launch.
#[tauri::command]
pub async fn check_for_update() -> Result<UpdateInfo, String> {
    tauri::async_runtime::spawn_blocking(update::check_blocking)
        .await
        .map_err(|e| e.to_string())?
}

/// Download + verify + swap the new binary, then restart into it. On success this
/// never returns (the process is replaced); errors bubble back to the banner.
#[tauri::command]
pub async fn run_update(app: tauri::AppHandle, info: UpdateInfo) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || update::download_and_swap(&info))
        .await
        .map_err(|e| e.to_string())??;
    app.restart()
}
