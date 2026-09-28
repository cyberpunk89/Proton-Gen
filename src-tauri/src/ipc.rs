//! Tauri command surface: a thin, serializable bridge over the pure logic
//! modules. Frontend selection state is the existing `store::Config`; the
//! catalog / recipes / runtimes / games / hardware are sent once via `bootstrap`.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

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
use crate::mangohud_export;
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
use crate::vkbasalt_export;

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
/// `last_played` / `playtime_minutes` come from `localconfig.vdf` for Steam
/// games (`steamlocate::App` has no such fields of its own) and from Heroic's
/// own `store/timestamp.json` for Heroic games — see [`game_dto`]. Both are
/// `None` for a game neither source has recorded yet, and always `None` for
/// non-Steam shortcuts (neither Steam nor Heroic tracks those).
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
    /// Box art hint for `source == "heroic"` — see `games::Game::art_url`.
    /// `game_art` looks it up from discovery; the webview never passes it back.
    pub art_url: Option<String>,
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
    /// Set only when a Steam install was found but zero Proton runtimes exist
    /// anywhere (system, this install's own compatibilitytools.d, or bundled
    /// Valve Proton). Same "name it, don't go silent" treatment as
    /// `load_error`, for a different failure mode.
    pub runtime_warning: Option<String>,
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

/// Shared application state: the cheap, always-available bits (catalog,
/// recipes, hardware, store) plus a lazily-filled filesystem discovery.
///
/// The immutable halves are behind `Arc` so an async command can hand a handle
/// to `spawn_blocking` instead of cloning a 100-entry catalog per call. Every
/// existing `&self.catalog` call site is unaffected — `Arc<Catalog>` derefs.
pub struct AppState {
    catalog: Arc<Catalog>,
    recipes: Arc<Recipes>,
    hardware: Hardware,
    /// Static warnings from the TOML overrides. Path warnings live on
    /// [`Discovery`] instead, because a rescan can clear them.
    config_warnings: Vec<ConfigWarning>,
    store: Arc<Mutex<Store>>,
    /// Serializes `save_store`'s disk writes. Each save runs on its own
    /// blocking thread, so without this two debounced saves could land out of
    /// order and leave the older store on disk.
    save_lock: Arc<Mutex<()>>,
    /// Filesystem discovery: `None` until the first `bootstrap` fills it,
    /// replaced wholesale by `rescan`.
    ///
    /// Deliberately **not** scanned in [`AppState::new`]. That runs inside
    /// `.manage(...)` *before* `tauri::Builder::run` creates a window, so every
    /// millisecond it spends enumerating Steam libraries, parsing
    /// `localconfig.vdf` and walking `$PATH` is a millisecond with nothing on
    /// screen at all — the app's own loading spinner cannot cover a window that
    /// does not exist yet. Deferring it to `bootstrap` (which runs off the main
    /// thread) is what lets that spinner do its job.
    discovery: Arc<Mutex<Option<Discovery>>>,
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
    ///
    /// Returns an owned clone rather than a borrow: the games now live behind a
    /// `Mutex`, so a reference would hold the guard for the caller's lifetime —
    /// and these callers `.await` afterwards, which a `MutexGuard` must not
    /// outlive.
    fn game(&self, app_id: u32) -> Option<GameDto> {
        let guard = self.discovery.lock().unwrap();
        guard.as_ref()?.games.iter().find(|g| g.app_id == app_id).cloned()
    }

    /// The Steam root from the last discovery pass, if any has run.
    fn steam_root(&self) -> Option<String> {
        self.discovery.lock().unwrap().as_ref()?.steam_root.clone()
    }

    /// Assemble the frontend payload from a discovery snapshot plus the static
    /// state. Shared by `bootstrap` and `rescan`, which differ only in whether
    /// they reuse the cached scan.
    fn bootstrap_from(&self, d: &Discovery, store: Store) -> Bootstrap {
        Bootstrap {
            steam_root: d.steam_root.clone(),
            load_error: d.load_error.clone(),
            runtime_warning: d.runtime_warning.clone(),
            catalog: (*self.catalog).clone(),
            categories: self.catalog.categories(),
            recipes: self.recipes.recipes.clone(),
            runtimes: d.runtimes.clone(),
            games: d.games.clone(),
            hardware: self.hardware.clone(),
            store,
            launch_options: d.launch_options.clone(),
            compat_tools: d.compat_tools.clone(),
            requires_status: d.requires_status.clone(),
            stale: d.stale.clone(),
            // Parse warnings plus whatever the scan made of the configured
            // paths, so a bad path is visible from the first frame.
            config_warnings: self
                .config_warnings
                .iter()
                .cloned()
                .chain(d.path_warnings.iter().cloned())
                .collect(),
        }
    }
}

/// Results of a filesystem re-scan: everything that can change while the app is
/// running (a game installed, a Proton runtime added). Produced by the first
/// `bootstrap` and replaced by every `rescan` — never by `AppState::new()`,
/// which must stay cheap (see [`AppState::discovery`]).
#[derive(Clone)]
struct Discovery {
    steam_root: Option<String>,
    load_error: Option<String>,
    runtime_warning: Option<String>,
    runtimes: Vec<RuntimeDto>,
    games: Vec<GameDto>,
    launch_options: HashMap<String, String>,
    compat_tools: HashMap<String, String>,
    /// required binary name -> whether it's on `$PATH`. Part of the scan rather
    /// than static state: a rescan is how a corrected binary override turns its
    /// badge green.
    requires_status: HashMap<String, bool>,
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
    let mut runtime_warning = None;
    let mut runtimes_raw = Vec::new();
    let mut games = Vec::new();
    let mut launch_options = HashMap::new();
    let mut compat_tools = HashMap::new();
    let mut path_warnings = Vec::new();

    // Heroic's own last-played/playtime, keyed by its `app_name` — read
    // unconditionally since sideloaded games don't need Steam to be installed.
    let heroic_playtime = heroic::load_playtime();

    match steam::locate_native(&paths.steam_roots, &mut path_warnings) {
        Ok(dir) => {
            steam_root = Some(steam::root_display(&dir));
            runtimes_raw = runtime::discover(&dir, &paths.proton_dirs, &mut path_warnings);
            if runtimes_raw.is_empty() {
                runtime_warning = Some(runtime::no_runtimes_message(
                    &steam::user_compat_tools_dir(&dir),
                    &paths.proton_dirs,
                ));
            }
            // localconfig first: `list_games_dto` reads last-played/playtime out
            // of it, so the parsed map has to exist before the games are built.
            let app_cfgs = steamcfg::current_app_cfgs(&dir, &mut path_warnings);
            games = list_games_dto(
                &dir,
                &app_cfgs,
                &heroic_playtime,
                &paths.steam_libraries,
                &mut path_warnings,
            );
            launch_options = stringify_keys(steamcfg::launch_options(&app_cfgs));
            compat_tools = stringify_keys(steamcfg::current_compat_tools(&dir));
        }
        Err(e) => {
            load_error = Some(e.to_string());
            // Heroic games don't need Steam. With no Steam install, `list_games`
            // never runs, so surface sideloaded Heroic games on their own here.
            games = games::dedup_and_sort(games::list_heroic_games())
                .into_iter()
                .map(|g| game_dto(g, &HashMap::new(), &heroic_playtime))
                .collect();
        }
    }

    let stale = compute_stale(catalog, &runtimes_raw);
    let runtimes = runtimes_raw.iter().map(runtime_dto).collect();
    let requires_status =
        compute_requires_status(catalog, &builder::Bins::with_overrides(&paths.bins));

    Discovery {
        steam_root,
        load_error,
        runtime_warning,
        runtimes,
        games,
        launch_options,
        compat_tools,
        requires_status,
        stale,
        path_warnings,
    }
}

impl AppState {
    pub fn new() -> Self {
        let (catalog, catalog_warning) = Catalog::load();
        let (recipes, recipes_warning) = Recipes::load();
        let (store, store_warning) = Store::load_or_recover();
        let config_warnings =
            catalog_warning.into_iter().chain(recipes_warning).chain(store_warning).collect();
        let hardware = hardware::detect();

        // No filesystem scan here — see `AppState::discovery`.
        Self {
            catalog: Arc::new(catalog),
            recipes: Arc::new(recipes),
            hardware,
            config_warnings,
            store: Arc::new(Mutex::new(store)),
            save_lock: Arc::new(Mutex::new(())),
            discovery: Arc::new(Mutex::new(None)),
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
/// Steam apps' last-played/playtime from `localconfig.vdf`; `heroic_playtime`
/// supplies the same pair for Heroic games from `store/timestamp.json`, keyed
/// by `heroic_id` rather than `app_id` (a Heroic game's `app_id` is a synthetic
/// hash for protongen's own bookkeeping, not an id Heroic itself knows about).
/// Non-Steam shortcuts have neither source and always get `None`.
fn game_dto(
    g: games::Game,
    app_cfgs: &HashMap<u32, steamcfg::AppUserCfg>,
    heroic_playtime: &HashMap<String, heroic::PlayStats>,
) -> GameDto {
    let (last_played, playtime_minutes) = match g.source {
        GameSource::Steam => {
            let cfg = app_cfgs.get(&g.app_id);
            (cfg.and_then(|c| c.last_played), cfg.and_then(|c| c.playtime_minutes))
        }
        GameSource::Heroic => {
            let stats = g.heroic_id.as_deref().and_then(|id| heroic_playtime.get(id));
            (stats.and_then(|s| s.last_played), stats.and_then(|s| s.playtime_minutes))
        }
        GameSource::NonSteam => (None, None),
    };
    GameDto {
        app_id: g.app_id,
        name: g.name,
        source: g.source.label().to_string(),
        executable: g.executable,
        installed: g.installed,
        last_played,
        playtime_minutes,
        heroic_id: g.heroic_id,
        install_dir: g.install_dir.map(|p| p.display().to_string()),
        art_url: g.art_url,
    }
}

fn list_games_dto(
    dir: &SteamDir,
    app_cfgs: &HashMap<u32, steamcfg::AppUserCfg>,
    heroic_playtime: &HashMap<String, heroic::PlayStats>,
    extra_libraries: &[String],
    warn: &mut Vec<ConfigWarning>,
) -> Vec<GameDto> {
    games::list_games(dir, extra_libraries, warn)
        .into_iter()
        .map(|g| game_dto(g, app_cfgs, heroic_playtime))
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
        // Badge the program that will actually be emitted, override included.
        let installed = crate::which::is_installed(bins.program(&bin));
        out.entry(bin).or_insert(installed);
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

/// Everything the frontend needs at startup, in one round-trip.
///
/// Runs the filesystem scan the first time it is called (off the UI thread) and
/// caches it; later calls reuse that snapshot, so a `Retry` after a failed
/// `init()` is cheap. `rescan` is the way to force a fresh look.
#[tauri::command]
pub async fn bootstrap(state: State<'_, AppState>) -> Result<Bootstrap, String> {
    let store = { state.store.lock().unwrap().clone() };
    let cached = { state.discovery.lock().unwrap().clone() };

    let d = match cached {
        Some(d) => d,
        None => {
            let catalog = Arc::clone(&state.catalog);
            let paths = store.paths.clone();
            let d = tauri::async_runtime::spawn_blocking(move || scan_discovery(&catalog, &paths))
                .await
                .map_err(|e| e.to_string())?;
            *state.discovery.lock().unwrap() = Some(d.clone());
            d
        }
    };

    Ok(state.bootstrap_from(&d, store))
}

/// Re-scan Steam / runtimes / games and return a fresh `Bootstrap` so the UI can
/// pick up newly-installed games or Proton runtimes without a restart. The
/// static fields (catalog, recipes, hardware) and the current store are reused
/// unchanged. `requires_status` and the path warnings are *not* static — a
/// rescan is how a corrected Settings path clears its banner and turns a binary
/// override's badge green. Runs off the UI thread: this is a full filesystem
/// scan, and it fires on a debounce while the user types a path into Settings.
///
/// `AppState::discovery` *is* replaced here, unlike the old flat snapshot —
/// `game_art` and the OptiScaler commands read it, so leaving it stale would
/// point them at a library the user just corrected.
#[tauri::command]
pub async fn rescan(state: State<'_, AppState>) -> Result<Bootstrap, String> {
    let store = { state.store.lock().unwrap().clone() };
    let catalog = Arc::clone(&state.catalog);
    let paths = store.paths.clone();

    let d = tauri::async_runtime::spawn_blocking(move || scan_discovery(&catalog, &paths))
        .await
        .map_err(|e| e.to_string())?;

    *state.discovery.lock().unwrap() = Some(d.clone());
    Ok(state.bootstrap_from(&d, store))
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
pub async fn inject_heroic(
    state: State<'_, AppState>,
    app_name: String,
    config: Config,
) -> Result<heroic::InjectResult, String> {
    // Only a game discovery actually found may be written to.
    let known = state.discovery.lock().unwrap().as_ref().is_some_and(|d| {
        d.games.iter().any(|g| g.heroic_id.as_deref() == Some(app_name.as_str()))
    });
    if !known {
        return Err(format!("no discovered Heroic game has the id {app_name:?}"));
    }
    let (env, wrappers) = compose::resolve_env_wrappers(&state.catalog, &config);
    let bins = state.bins();
    tauri::async_runtime::spawn_blocking(move || heroic::inject(&app_name, &env, &wrappers, &bins))
        .await
        .map_err(|e| e.to_string())?
}

/// Best-effort check for a running Heroic process, so the "Apply to Heroic?"
/// confirmation can warn before writing — see [`heroic::is_running`].
#[tauri::command]
pub async fn heroic_running() -> bool {
    tauri::async_runtime::spawn_blocking(heroic::is_running).await.unwrap_or(false)
}

/// Parse a pasted Steam/umu command into a `Config` (unknown env → extra_env).
#[tauri::command]
pub fn parse_command(state: State<'_, AppState>, input: String) -> ParsedCommand {
    let catalog = &state.catalog;
    let p = parser::parse(&input, &catalog.plain_wrappers());

    // Reconstruct wrapper key/value list from the parsed wrappers.
    let wrappers: Vec<(String, String)> = p
        .wrappers()
        .iter()
        .map(|w| match w {
            Wrapper::Gamescope(a) => ("gamescope".to_string(), a.clone()),
            Wrapper::GamePerformance => ("game-performance".to_string(), String::new()),
            Wrapper::Gamemoderun => ("gamemoderun".to_string(), String::new()),
            Wrapper::Mangohud => ("mangohud".to_string(), String::new()),
            Wrapper::Plain(p) => (p.key.clone(), String::new()),
        })
        .collect();

    // Enable catalog-known env/wrappers; capture them back in catalog order.
    // Anything the catalog doesn't know comes back as a leftover and goes to the
    // custom-env field, which is what makes the import lossless.
    let (options, unknown) = store::options_from_lists(catalog, &p.env, &wrappers);
    let (env, wrappers) = store::options_to_lists(catalog, &options);

    // `Parsed::unknown` also flags an assignment the shell never exported
    // (`A=x;y`) — that one *was* imported, as env, so it isn't reported here.
    let imported = |t: &String| t.split_once('=').is_some_and(|(k, _)| p.env.iter().any(|(e, _)| e == k));
    let dropped = p.unknown.iter().filter(|t| !imported(t)).cloned().collect();

    ParsedCommand {
        config: Config {
            umu: p.umu,
            runtime: None,
            env,
            wrappers,
            extra_env: compose::format_extra_env(&unknown),
            umu_exe: p.umu_exe,
            umu_wineprefix: p.umu_wineprefix.unwrap_or_default(),
            umu_gameid: p.umu_gameid.unwrap_or_default(),
            game_args: p.game_args,
        },
        dropped,
    }
}

/// A pasted command read back into a `Config`, plus what couldn't be.
#[derive(Clone, Serialize)]
pub struct ParsedCommand {
    pub config: Config,
    /// Pre-target tokens with no place in a `Config` — a foreign wrapper
    /// (`strangle`), its flags, `PROTONPATH=` in Steam mode. Reported so an
    /// import never silently loses part of the command.
    pub dropped: Vec<String>,
}

/// Tokenize a launch command for the annotated preview. Tokens carry only a
/// catalog `key`, which the frontend resolves against the already-loaded
/// catalog; the state is read only for the catalog's plain wrappers.
#[tauri::command]
pub fn explain_command(state: State<'_, AppState>, command: String) -> Vec<Token> {
    explain::explain(&command, &state.catalog.plain_wrappers())
}

/// Steam's per-game launch options and compat-tool mapping, freshly read.
#[derive(Clone, Serialize)]
pub struct SteamUserConfig {
    pub launch_options: HashMap<String, String>,
    pub compat_tools: HashMap<String, String>,
}

/// Re-read only what pasting into Steam changes — launch options and the
/// compat-tool mapping — without a full library scan, so the window regaining
/// focus can refresh the sync verdict cheaply. `None` without a Steam install.
/// Also updates the cached discovery, so a later `bootstrap` (Retry) agrees.
#[tauri::command]
pub async fn steam_user_config(
    state: State<'_, AppState>,
) -> Result<Option<SteamUserConfig>, String> {
    let paths = state.store.lock().unwrap().paths.clone();
    let fresh = tauri::async_runtime::spawn_blocking(move || {
        let dir = steam::locate_native(&paths.steam_roots, &mut Vec::new()).ok()?;
        // Warnings belong to the full scan's banner; this re-read is silent.
        let cfgs = steamcfg::current_app_cfgs(&dir, &mut Vec::new());
        Some(SteamUserConfig {
            launch_options: stringify_keys(steamcfg::launch_options(&cfgs)),
            compat_tools: stringify_keys(steamcfg::current_compat_tools(&dir)),
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    if let (Some(cfg), Some(d)) = (&fresh, state.discovery.lock().unwrap().as_mut()) {
        d.launch_options = cfg.launch_options.clone();
        d.compat_tools = cfg.compat_tools.clone();
    }
    Ok(fresh)
}

/// Compare a built launch command against the one Steam currently has set.
/// Stateless and pure — see `diff.rs` for what is deliberately normalised away.
///
/// The *contextual* states (no game selected, non-Steam shortcut, generic
/// command) stay out of the DTO: the frontend already holds `selectedAppId`,
/// `game.source` and `app.umu`, and pushing them here would drag game/mode
/// state through an otherwise trivially pure function.
#[tauri::command]
pub fn launch_diff(state: State<'_, AppState>, built: String, current: String) -> LaunchDiff {
    diff::compare(&built, &current, &state.catalog.plain_wrappers())
}

/// Applied / drifted / not-applied for every remembered game in one call, so
/// the library grid can badge them all at a glance.
///
/// `launch_options` comes from the frontend rather than `AppState`: the
/// frontend holds the freshest copy as `app.launchOptions` (a focus refresh can
/// update it between scans), and passing it in keeps `diff::statuses` a pure
/// function of its arguments.
#[tauri::command]
pub async fn launch_statuses(
    state: State<'_, AppState>,
    memory: BTreeMap<String, Config>,
    launch_options: HashMap<String, String>,
) -> Result<HashMap<String, diff::DiffStatus>, String> {
    // Off the UI thread: this re-assembles, re-parses and diffs one command per
    // remembered game, so its cost grows with the user's library.
    let catalog = Arc::clone(&state.catalog);
    let bins = state.bins();
    tauri::async_runtime::spawn_blocking(move || {
        diff::statuses(&catalog, &memory, &launch_options, &bins)
    })
    .await
    .map_err(|e| e.to_string())
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
    // The declared AMD generation is a store field, so it has to be read off the
    // store rather than off `state.hardware` — but with the same fallback to
    // detection the frontend's `effectiveGpuGen` uses. Without it these rules and
    // the visibility filter would disagree about which generation is in force,
    // and a user who never touched the Settings selector would see RDNA4 rows
    // while the RDNA4 lint notices stayed silent.
    let gpu_gen = lint::effective_gpu_gen(
        &state.store.lock().unwrap().gpu_gen,
        state.hardware.gpu_gen_detected.as_deref(),
        state.hardware.amd,
    );
    let mut notices = lint::warnings(&state.catalog, &options, &state.hardware, &gpu_gen);
    notices.extend(lint::invalid_custom_env(&compose::invalid_extra_env(&config.extra_env)));
    notices
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
///
/// A Heroic sideload has no Steam appid a cache lookup could key off, so its
/// own `art_cover` / `art_square` (a `file://` path or remote URL) is the only
/// way to find its art. That hint is looked up here from discovery, never
/// taken from the webview: it is read off disk or fetched as given, so a
/// caller-supplied one would read any file or fetch any URL.
#[tauri::command]
pub async fn game_art(
    state: State<'_, AppState>,
    app_id: u32,
    source: String,
    kind: String,
    online: bool,
) -> Result<Option<String>, String> {
    // Both end up in a cache file name, so only the known values pass.
    if !art::SOURCES.contains(&source.as_str()) || !art::KINDS.contains(&kind.as_str()) {
        return Err(format!("unknown art source/kind: {source}/{kind}"));
    }
    let art_hint = state
        .game(app_id)
        .filter(|g| g.source == source)
        .and_then(|g| g.art_url);
    let steam_root = state.steam_root();
    tauri::async_runtime::spawn_blocking(move || {
        art::fetch(steam_root, app_id, &source, &kind, online, art_hint)
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
///
/// The in-memory swap happens synchronously — `lint` reads `gpu_gen` straight
/// off the store, so it must never observe the old value after this returns —
/// while the TOML serialization and the disk write go to a blocking thread. This
/// fires on a debounce *while the user types* in the builder, so on the main
/// thread it was a write of the entire store between keystrokes.
#[tauri::command]
pub async fn save_store(state: State<'_, AppState>, store: Store) -> Result<(), String> {
    *state.store.lock().unwrap() = store;
    let current = Arc::clone(&state.store);
    let save_lock = Arc::clone(&state.save_lock);
    tauri::async_runtime::spawn_blocking(move || {
        let _serial = save_lock.lock().unwrap();
        // Snapshot under the lock, not before it: a save that lost the race
        // then writes the newest store instead of rolling the file back.
        let snapshot = current.lock().unwrap().clone();
        snapshot.save()
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Whether `app_id` already has an OptiScaler install to refresh — cheap
/// filesystem check, no network. `install_dir` comes from `AppState`'s own
/// discovery, never from the caller (see `AppState::game`).
#[tauri::command]
pub async fn optiscaler_status(
    state: State<'_, AppState>,
    app_id: u32,
) -> Result<optiscaler_upgrade::OptiscalerStatus, String> {
    let dir = state.game(app_id).and_then(|g| g.install_dir);
    tauri::async_runtime::spawn_blocking(move || {
        optiscaler_upgrade::detect(dir.as_deref().map(std::path::Path::new))
    })
    .await
    .map_err(|e| e.to_string())
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
        .and_then(|g| g.install_dir)
        .ok_or_else(|| "no install directory known for this game".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        optiscaler_upgrade::fetch_and_extract(std::path::Path::new(&dir))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Write `config` (a `MANGOHUD_CONFIG`-style string) into the real, system-wide
/// `~/.config/MangoHud/MangoHud.conf`, merging it with whatever's already there.
/// The third sanctioned write outside protongen's own state — see
/// `mangohud_export`'s doc comment. Backs the file up first if it existed;
/// never gated here, only ever called from the frontend's confirm dialog.
#[tauri::command]
pub async fn export_mangohud_system(config: String) -> Result<mangohud_export::ExportResult, String> {
    tauri::async_runtime::spawn_blocking(move || mangohud_export::write_system_config(&config))
        .await
        .map_err(|e| e.to_string())?
}

/// Read the real, system-wide `~/.config/vkBasalt/vkBasalt.conf`'s current
/// text, so the vkBasalt builder dialog can seed itself from it rather than
/// risk clobbering a hand-tuned file. Empty string if the file doesn't exist
/// yet — read-only, no write involved.
#[tauri::command]
pub async fn vkbasalt_read_config() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(vkbasalt_export::read_current_config)
        .await
        .map_err(|e| e.to_string())?
}

/// Write `config` (a newline-delimited `key = value` block) into the real,
/// system-wide `~/.config/vkBasalt/vkBasalt.conf`, merging it with whatever's
/// already there. The fourth sanctioned write outside protongen's own state —
/// see `vkbasalt_export`'s doc comment. Backs the file up first if it
/// existed; never gated here, only ever called from the frontend's confirm
/// dialog.
#[tauri::command]
pub async fn export_vkbasalt_system(config: String) -> Result<vkbasalt_export::ExportResult, String> {
    tauri::async_runtime::spawn_blocking(move || vkbasalt_export::write_system_config(&config))
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
///
/// Takes no arguments on purpose: the release is re-resolved here rather than
/// trusted from the webview, which could otherwise pick both the binary and
/// the checksum it is verified against.
#[tauri::command]
pub async fn run_update(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| update::download_and_swap(&update::check_blocking()?))
        .await
        .map_err(|e| e.to_string())??;
    app.restart()
}
