//! Tauri command surface: a thin, serializable bridge over the pure logic
//! modules. Frontend selection state is the existing `store::Config`; the
//! catalog / recipes / runtimes / games / hardware are sent once via `bootstrap`.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use steamlocate::SteamDir;
use tauri::State;

use crate::art;
use crate::builder::Wrapper;
use crate::compose;
use crate::explain::{self, Token};
use crate::games::{self, GameSource};
use crate::hardware::{self, Hardware};
use crate::lint;
use crate::params::{Catalog, Options};
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
#[derive(Clone, Serialize)]
pub struct GameDto {
    pub app_id: u32,
    pub name: String,
    pub source: String,
    pub executable: Option<String>,
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
    store: Mutex<Store>,
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
}

/// Re-run Steam / runtime / games discovery. Pure and idempotent — safe to call
/// repeatedly. `catalog` is only read (for staleness); it never changes here.
fn scan_discovery(catalog: &Catalog) -> Discovery {
    let mut steam_root = None;
    let mut load_error = None;
    let mut runtimes_raw = Vec::new();
    let mut games = Vec::new();
    let mut launch_options = HashMap::new();
    let mut compat_tools = HashMap::new();

    match steam::locate_native() {
        Ok(dir) => {
            steam_root = Some(steam::root_display(&dir));
            runtimes_raw = runtime::discover(&dir);
            games = list_games_dto(&dir);
            launch_options = stringify_keys(steamcfg::current_launch_options(&dir));
            compat_tools = stringify_keys(steamcfg::current_compat_tools(&dir));
        }
        Err(e) => load_error = Some(e.to_string()),
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
    }
}

impl AppState {
    pub fn new() -> Self {
        let catalog = Catalog::load();
        let recipes = Recipes::load();
        let hardware = hardware::detect();
        let store = Store::load();

        let d = scan_discovery(&catalog);
        let requires_status = compute_requires_status(&catalog);

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
            store: Mutex::new(store),
        }
    }
}

fn runtime_dto(r: &runtime::Runtime) -> RuntimeDto {
    let kind = match r.kind {
        RuntimeKind::System => "system",
        RuntimeKind::User => "user",
        RuntimeKind::Bundled => "valve",
    };
    RuntimeDto {
        internal_name: r.internal_name.clone(),
        display_name: r.display_name.clone(),
        kind: kind.to_string(),
        path: r.path.display().to_string(),
    }
}

fn list_games_dto(dir: &SteamDir) -> Vec<GameDto> {
    games::list_games(dir)
        .into_iter()
        .map(|g| GameDto {
            app_id: g.app_id,
            name: g.name,
            source: match g.source {
                GameSource::Steam => "steam",
                GameSource::NonSteam => "non-steam",
            }
            .to_string(),
            executable: g.executable,
        })
        .collect()
}

fn stringify_keys(m: HashMap<u32, String>) -> HashMap<String, String> {
    m.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// For every distinct `requires` binary in the catalog, whether it's on $PATH.
fn compute_requires_status(catalog: &Catalog) -> HashMap<String, bool> {
    let mut out = HashMap::new();
    let bins = catalog
        .wrappers
        .iter()
        .filter_map(|w| w.requires.clone())
        .chain(catalog.envs.iter().filter_map(|e| e.requires.clone()));
    for bin in bins {
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
        hardware: state.hardware,
        store,
        launch_options: state.launch_options.clone(),
        compat_tools: state.compat_tools.clone(),
        requires_status: state.requires_status.clone(),
        stale: state.stale.clone(),
    }
}

/// Re-scan Steam / runtimes / games and return a fresh `Bootstrap` so the UI can
/// pick up newly-installed games or Proton runtimes without a restart. The
/// static fields (catalog, recipes, hardware, requires_status) and the current
/// store are reused unchanged. `AppState`'s discovery snapshot is intentionally
/// left untouched — no command reads it after startup, so there's nothing to keep
/// in sync (build_command/lint work off the passed Config + catalog).
#[tauri::command]
pub fn rescan(state: State<'_, AppState>) -> Bootstrap {
    let store = state.store.lock().unwrap().clone();
    let d = scan_discovery(&state.catalog);
    Bootstrap {
        steam_root: d.steam_root,
        load_error: d.load_error,
        catalog: state.catalog.clone(),
        categories: state.catalog.categories(),
        recipes: state.recipes.recipes.clone(),
        runtimes: d.runtimes,
        games: d.games,
        hardware: state.hardware,
        store,
        launch_options: d.launch_options,
        compat_tools: d.compat_tools,
        requires_status: state.requires_status.clone(),
        stale: d.stale,
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
    compose::assemble(&state.catalog, &config, proton_path.as_deref())
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
            Wrapper::Gamemoderun => ("gamemoderun".to_string(), String::new()),
            Wrapper::Mangohud => ("mangohud".to_string(), String::new()),
        })
        .collect();

    // Enable catalog-known env/wrappers; capture them back in catalog order.
    let mut options = Options::from_catalog(catalog);
    store::apply_lists(catalog, &mut options, &p.env, &wrappers);
    let (env, wrappers) = store::options_to_lists(catalog, &options);

    Config {
        umu: p.umu,
        runtime: None,
        env,
        wrappers,
        extra_env: store::unknown_env_string(catalog, &p.env),
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

/// Merge recipe `index` onto `config`, returning the updated config.
#[tauri::command]
pub fn apply_recipe(state: State<'_, AppState>, index: usize, config: Config) -> Config {
    let catalog = &state.catalog;
    let Some(recipe) = state.recipes.recipes.get(index) else {
        return config;
    };

    let mut options = compose::options_from_config(catalog, &config);
    let mut extra_env = config.extra_env.clone();
    recipes::apply(recipe, catalog, &mut options, &mut extra_env);

    let (env, wrappers) = store::options_to_lists(catalog, &options);
    Config {
        env,
        wrappers,
        extra_env,
        ..config
    }
}

/// Conflict / footgun notices for the current config.
#[tauri::command]
pub fn lint(state: State<'_, AppState>, config: Config) -> Vec<String> {
    let options = compose::options_from_config(&state.catalog, &config);
    lint::warnings(&state.catalog, &options, &state.hardware)
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

/// Replace and persist the whole store (theme, presets, per-game memory, dismissals).
#[tauri::command]
pub fn save_store(state: State<'_, AppState>, store: Store) {
    let mut guard = state.store.lock().unwrap();
    *guard = store;
    guard.save();
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
