//! Tauri command surface: a thin, serializable bridge over the pure logic
//! modules. Frontend selection state is the existing `store::Config`; the
//! catalog / recipes / runtimes / games / hardware are sent once via `bootstrap`.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::Serialize;
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
use crate::lsfg;
use crate::llm::{self, LlmRequest, LlmSuggestion, RecipeRef, TroubleshootRequest, TroubleshootResult};
use crate::mangohud_export;
use crate::optiscaler_upgrade;
use crate::runtime_updates;
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

/// `lock()` that shrugs off poisoning. Everything behind these mutexes is
/// plain data replaced wholesale (store snapshot, discovery cache, a unit
/// for save ordering), so a panic mid-hold can't leave it half-updated in a
/// way that matters — but with `.unwrap()` that one panic would make every
/// later command touching the mutex panic too, `save_store` included, for
/// the rest of the session.
trait Locked<T> {
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T> Locked<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A runtime, flattened for the frontend (path + kind as strings).
#[derive(Clone, Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
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
#[derive(Clone, Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
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
    /// Nexus's key for the game; `Some` only for `source == "nexus"`. What
    /// `apply_to_nexus` addresses it by.
    pub nexus_slug: Option<String>,
    /// The game's own Wine prefix (Nexus games) — prefilled into umu mode.
    pub wine_prefix: Option<String>,
    /// The Proton build Nexus runs it with, by folder name.
    pub pinned_proton: Option<String>,
    /// Ids this entry replaced (a Nexus game's Steam-shortcut / Heroic
    /// mirrors), best source of saved tuning first — the frontend carries
    /// `game_memory` saved under them over to `app_id`.
    pub alias_ids: Vec<u32>,
}

pub use crate::logs::ProtonLog;

/// The "catalog refreshed for an older build" banner data.
#[derive(Clone, Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct StaleInfo {
    pub installed: String,
    pub catalog: String,
    pub updated: String,
}

/// Everything the frontend needs at startup, in one round-trip.
#[derive(Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
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
    /// The game to open on, from `protongen --game <appid>`. Only the first
    /// bootstrap carries it; a rescan never re-selects.
    pub initial_game_appid: Option<u32>,
    /// The discovery half came from the last session's cache (see
    /// `discovery_cache`), not a fresh scan: the frontend should `rescan` in
    /// the background right away.
    pub from_cache: bool,
}

/// What a `rescan` sends back: only the fields a filesystem scan can change.
/// The catalog, recipes, hardware and store are static for the session (or
/// owned by the frontend), so re-sending them — the catalog alone is ~100 KB
/// of JSON — on every rescan, which fires on a debounce while a Settings path
/// is being typed, was pure serialization overhead.
#[derive(Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct Scan {
    pub steam_root: Option<String>,
    pub load_error: Option<String>,
    pub runtime_warning: Option<String>,
    pub runtimes: Vec<RuntimeDto>,
    pub games: Vec<GameDto>,
    pub launch_options: HashMap<String, String>,
    pub compat_tools: HashMap<String, String>,
    pub requires_status: HashMap<String, bool>,
    pub stale: Option<StaleInfo>,
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
    /// `--game <appid>` from the command line, taken by the first bootstrap.
    initial_game: Arc<Mutex<Option<u32>>>,
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
    /// Art files `game_art` resolved, by the opaque key the webview loads them
    /// with (`art://localhost/<key>`). The `art` protocol serves only these —
    /// a URL never carries a filesystem path.
    pub(crate) art_files: Arc<Mutex<HashMap<String, std::path::PathBuf>>>,
}

impl AppState {
    /// Wrapper program names, with the user's Settings overrides applied.
    ///
    /// Built per call rather than cached: `save_store` can replace the whole
    /// store mid-session, and a cached copy would stay stale until restart.
    fn bins(&self) -> builder::Bins {
        builder::Bins::with_overrides(&self.store.locked().paths.bins)
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
        let guard = self.discovery.locked();
        guard.as_ref()?.games.iter().find(|g| g.app_id == app_id).cloned()
    }

    /// The Steam root from the last discovery pass, if any has run.
    fn steam_root(&self) -> Option<String> {
        self.discovery.locked().as_ref()?.steam_root.clone()
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
            initial_game_appid: None,
            from_cache: false,
        }
    }
}

impl AppState {
    /// The rescan payload: [`Self::bootstrap_from`] minus the static parts.
    fn scan_from(&self, d: &Discovery) -> Scan {
        Scan {
            steam_root: d.steam_root.clone(),
            load_error: d.load_error.clone(),
            runtime_warning: d.runtime_warning.clone(),
            runtimes: d.runtimes.clone(),
            games: d.games.clone(),
            launch_options: d.launch_options.clone(),
            compat_tools: d.compat_tools.clone(),
            requires_status: d.requires_status.clone(),
            stale: d.stale.clone(),
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
#[derive(Clone, Serialize, serde::Deserialize)]
pub(crate) struct Discovery {
    pub(crate) steam_root: Option<String>,
    load_error: Option<String>,
    runtime_warning: Option<String>,
    pub(crate) runtimes: Vec<RuntimeDto>,
    pub(crate) games: Vec<GameDto>,
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
pub(crate) fn scan_discovery(catalog: &Catalog, paths: &store::Paths) -> Discovery {
    // The sources are independent files (Steam's VDFs, each library's
    // appmanifests, Heroic's JSON, Nexus's games.json, `$PATH`), so they are
    // read on scoped threads rather than one after another. Warnings are still
    // pushed in the old fixed order — runtimes, Steam config, libraries — so
    // the banner reads the same however the threads finish.
    std::thread::scope(|s| {
        // Heroic's own last-played/playtime, keyed by its `app_name` — read
        // unconditionally since sideloaded games don't need Steam installed.
        let heroic_playtime = s.spawn(heroic::load_playtime);
        let requires_status = s.spawn(|| {
            compute_requires_status(catalog, &builder::Bins::with_overrides(&paths.bins))
        });

        let mut path_warnings = Vec::new();
        let mut steam_root = None;
        let mut load_error = None;
        let mut runtime_warning = None;
        let mut runtimes_raw = Vec::new();
        let mut launch_options = HashMap::new();
        let mut compat_tools = HashMap::new();
        let games;

        match steam::locate_native(&paths.steam_roots, &mut path_warnings) {
            Ok(dir) => {
                steam_root = Some(steam::root_display(&dir));
                let dir = &dir;
                // An inner scope: these borrow `dir`, which lives only in this arm.
                let ((rt, rt_warn), (app_cfgs, cfg_warn), compat, raw_games, library_warnings) =
                    std::thread::scope(|s| {
                        let runtimes = s.spawn(|| {
                            let mut warn = Vec::new();
                            (runtime::discover(dir, &paths.proton_dirs, &mut warn), warn)
                        });
                        let cfgs = s.spawn(|| {
                            let mut warn = Vec::new();
                            (steamcfg::current_app_cfgs(dir, &mut warn), warn)
                        });
                        let compat = s.spawn(|| steamcfg::current_compat_tools(dir));
                        let mut warn = Vec::new();
                        let raw_games = games::list_games(dir, &paths.steam_libraries, &mut warn);
                        (
                            runtimes.join().expect("runtime discovery panicked"),
                            cfgs.join().expect("localconfig scan panicked"),
                            compat.join().expect("compat tool scan panicked"),
                            raw_games,
                            warn,
                        )
                    });

                runtimes_raw = rt;
                path_warnings.extend(rt_warn);
                if runtimes_raw.is_empty() {
                    runtime_warning = Some(runtime::no_runtimes_message(
                        &steam::user_compat_tools_dir(dir),
                        &paths.proton_dirs,
                    ));
                }
                // `game_dto` reads last-played/playtime out of localconfig, so
                // the parsed map is needed before the games are built.
                path_warnings.extend(cfg_warn);
                path_warnings.extend(library_warnings);
                let heroic_playtime = heroic_playtime.join().expect("heroic playtime panicked");
                games = raw_games
                    .into_iter()
                    .map(|g| game_dto(g, &app_cfgs, &heroic_playtime))
                    .collect();
                launch_options = stringify_keys(steamcfg::launch_options(&app_cfgs));
                compat_tools = stringify_keys(compat);
            }
            Err(e) => {
                load_error = Some(e.to_string());
                // Heroic games don't need Steam. With no Steam install,
                // `list_games` never runs, so surface sideloaded Heroic games
                // on their own here.
                let heroic_playtime = heroic_playtime.join().expect("heroic playtime panicked");
                games = games::dedup_and_sort(crate::nexus::absorb(
                    games::list_heroic_games(),
                    crate::nexus::list(),
                ))
                .into_iter()
                .map(|g| game_dto(g, &HashMap::new(), &heroic_playtime))
                .collect();
            }
        }

        let stale = compute_stale(catalog, &runtimes_raw);
        let runtimes = runtimes_raw.iter().map(runtime_dto).collect();

        Discovery {
            steam_root,
            load_error,
            runtime_warning,
            runtimes,
            games,
            launch_options,
            compat_tools,
            requires_status: requires_status.join().expect("PATH scan panicked"),
            stale,
            path_warnings,
        }
    })
}

impl AppState {
    pub fn new() -> Self {
        // This runs before the window exists, so it is on the time-to-first-
        // paint path. Hardware detection is all sysfs/procfs/`pci.ids` reads
        // and shares nothing with the TOML parsing, so it overlaps with it.
        let (hardware, (catalog, catalog_warning), (recipes, recipes_warning), (store, store_warning)) =
            std::thread::scope(|s| {
                let hw = s.spawn(hardware::detect);
                let loaded = (Catalog::load(), Recipes::load(), Store::load_or_recover());
                (hw.join().unwrap_or_default(), loaded.0, loaded.1, loaded.2)
            });
        let config_warnings =
            catalog_warning.into_iter().chain(recipes_warning).chain(store_warning).collect();

        // No filesystem scan here — see `AppState::discovery`.
        Self {
            catalog: Arc::new(catalog),
            recipes: Arc::new(recipes),
            hardware,
            config_warnings,
            store: Arc::new(Mutex::new(store)),
            save_lock: Arc::new(Mutex::new(())),
            initial_game: Arc::new(Mutex::new(None)),
            discovery: Arc::new(Mutex::new(None)),
            art_files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Open on this game once the UI boots (see `Bootstrap::initial_game_appid`).
    pub fn with_initial_game(self, appid: Option<u32>) -> Self {
        *self.initial_game.locked() = appid;
        self
    }

    /// A second `protongen --game <id>` arrived while this one runs. Parked in
    /// the same slot as the startup request, so one that lands before the UI
    /// has bootstrapped (and before it listens for `open-game`) still opens.
    pub fn request_game(&self, appid: u32) {
        *self.initial_game.locked() = Some(appid);
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
        // Nexus tracks its own sessions; time played through the Heroic
        // sideload it mirrors the game into adds to that, as Nexus counts it.
        GameSource::Nexus => {
            let info = g.nexus.as_ref();
            let heroic = info
                .and_then(|i| i.heroic_app_name.as_deref())
                .and_then(|id| heroic_playtime.get(id));
            let last = [info.and_then(|i| i.last_played), heroic.and_then(|s| s.last_played)]
                .into_iter()
                .flatten()
                .max();
            let minutes = [info.and_then(|i| i.playtime_minutes), heroic.and_then(|s| s.playtime_minutes)]
                .into_iter()
                .flatten()
                .reduce(u32::saturating_add);
            (last, minutes)
        }
    };
    let nexus = g.nexus.unwrap_or_default();
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
        nexus_slug: (g.source == GameSource::Nexus).then_some(nexus.slug),
        wine_prefix: nexus.wine_prefix,
        pinned_proton: nexus.proton,
        alias_ids: nexus.alias_ids,
    }
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
    let store = { state.store.locked().clone() };
    let cached = { state.discovery.locked().clone() };

    let (d, from_cache) = match cached {
        Some(d) => (d, false),
        None => {
            let catalog = Arc::clone(&state.catalog);
            let paths = store.paths.clone();
            // Last session's scan first: painting from it costs one small JSON
            // read, and the frontend re-scans behind it immediately. Only with
            // no usable cache does the first frame wait for a full scan.
            let (d, from_cache) = tauri::async_runtime::spawn_blocking(move || {
                match crate::discovery_cache::load(&paths) {
                    Some(d) => (d, true),
                    None => {
                        let d = scan_discovery(&catalog, &paths);
                        crate::discovery_cache::save(&d, &paths);
                        (d, false)
                    }
                }
            })
            .await
            .map_err(|e| e.to_string())?;
            *state.discovery.locked() = Some(d.clone());
            (d, from_cache)
        }
    };

    let mut b = state.bootstrap_from(&d, store);
    b.initial_game_appid = state.initial_game.locked().take();
    b.from_cache = from_cache;
    Ok(b)
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
pub async fn rescan(state: State<'_, AppState>) -> Result<Scan, String> {
    let catalog = Arc::clone(&state.catalog);
    let paths = state.store.locked().paths.clone();

    let d = tauri::async_runtime::spawn_blocking(move || {
        let d = scan_discovery(&catalog, &paths);
        crate::discovery_cache::save(&d, &paths);
        d
    })
    .await
    .map_err(|e| e.to_string())?;

    let scan = state.scan_from(&d);
    *state.discovery.locked() = Some(d);
    Ok(scan)
}

/// Everything the command bar shows for one edit, from one round trip: the
/// command, its annotated tokens, the lint notices, and — when there is a Steam
/// entry to compare against — the sync verdict. These used to be four
/// sequential IPC calls per keystroke; each reran the same catalog lookups and
/// paid its own WebKit↔Rust hop.
#[derive(Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct Recompute {
    pub command: String,
    pub tokens: Vec<Token>,
    pub notices: Vec<lint::Notice>,
    /// Set when lint itself failed; the command and tokens are still good, so
    /// a lint failure must never blank them.
    pub lint_error: Option<String>,
    /// `None` when `current` was not given (no game, shortcut, umu, generic).
    pub diff: Option<LaunchDiff>,
}

/// Assemble the launch command for the given config, tokenize it, lint it, and
/// diff it against `current` (Steam's launch options, when there are any).
/// `proton_path` is the selected runtime's install dir (PROTONPATH in umu
/// mode); `app_id` is the selected game, see [`lint_notices`].
///
/// The diff is *contextual* only through `current`: the frontend already holds
/// `selectedAppId`, `game.source` and `app.umu`, and decides whether there is
/// anything to compare, which keeps `diff::compare` a pure function.
#[tauri::command]
pub async fn recompute(
    state: State<'_, AppState>,
    config: Config,
    proton_path: Option<String>,
    app_id: Option<u32>,
    current: Option<String>,
) -> Result<Recompute, String> {
    // Built per call rather than cached on AppState: `save_store` can replace
    // the store mid-session, and a cached copy would stay stale until restart.
    // Cheap — this command is already debounced ~60 ms on the frontend.
    let bins = state.bins();
    let command = compose::assemble(&state.catalog, &config, proton_path.as_deref(), &bins);
    let plain = state.catalog.plain_wrappers();
    let tokens = explain::explain(&command, &plain);
    let diff = current.map(|current| diff::compare(&command, &current, &plain));
    let (notices, lint_error) = match lint_notices(&state, &config, app_id).await {
        Ok(n) => (n, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    Ok(Recompute { command, tokens, notices, lint_error, diff })
}

/// A game's Wine prefix and shader cache (see [`crate::folders`]); with
/// `sizes`, also walks them for their size — off the UI thread, and only
/// when the user asks.
#[tauri::command]
pub async fn game_folders(
    state: State<'_, AppState>,
    app_id: u32,
    sizes: bool,
) -> Result<Vec<crate::folders::Folder>, String> {
    let Some(game) = state.game(app_id) else {
        return Ok(Vec::new());
    };
    let root = state.steam_root().map(std::path::PathBuf::from);
    tauri::async_runtime::spawn_blocking(move || crate::folders::measure(&game, root.as_deref(), sizes))
        .await
        .map_err(|e| e.to_string())
}

/// Open one of a game's folders (`kind`: `prefix` | `shadercache`) in the
/// file manager. The path is recomputed here from discovery — the webview
/// names a kind, never a path — which is why this goes through Rust rather
/// than a frontend `openPath` with a filesystem-wide capability scope.
#[tauri::command]
pub fn open_game_folder(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    app_id: u32,
    kind: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let game = state.game(app_id).ok_or("unknown game")?;
    let root = state.steam_root().map(std::path::PathBuf::from);
    let (_, path) = crate::folders::locate(&game, root.as_deref())
        .into_iter()
        .find(|(k, _)| *k == kind)
        .ok_or_else(|| format!("no {kind} folder for this game"))?;
    if !path.is_dir() {
        return Err(format!("{} doesn't exist yet", path.display()));
    }
    app.opener().open_path(path.display().to_string(), None::<&str>).map_err(|e| e.to_string())
}

/// Hand the current tuning to Nexus for one of its games: runs
/// `nexus-cli --set-launch <slug> <code>`, where `code` is the config as a
/// `protongen:v1:` share code (the frontend's `encodePreset`, which already
/// strips the launch target). Nexus saves it as the game's launch profile
/// and re-applies it to launch.sh and the Steam/Heroic entries it maintains.
///
/// Explicit-confirm only (the frontend's NexusConfirm). The slug comes from
/// discovery, never from the caller, so this can only ever address a game
/// Nexus itself listed; the code must be share-code shaped. Nexus owns every
/// file it touches — protongen writes nothing here itself.
#[tauri::command]
pub async fn apply_to_nexus(
    state: State<'_, AppState>,
    app_id: u32,
    code: String,
) -> Result<String, String> {
    let game = state.game(app_id).filter(|g| g.source == "nexus");
    let slug = game.and_then(|g| g.nexus_slug).ok_or("not a Nexus game")?;
    if !crate::nexus::is_share_code(&code) {
        return Err("not a protongen share code".into());
    }
    let cli = crate::nexus::cli_path().ok_or("nexus-cli isn't installed (looked on PATH and in ~/.local/bin)")?;
    tauri::async_runtime::spawn_blocking(move || crate::nexus::set_launch(&cli, &slug, &code))
        .await
        .map_err(|e| e.to_string())?
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
    let known = state.discovery.locked().as_ref().is_some_and(|d| {
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
    parse_command_with(&state.catalog, &input)
}

/// [`parse_command`]'s logic, without the Tauri state — what the tests drive.
fn parse_command_with(catalog: &Catalog, input: &str) -> ParsedCommand {
    let p = parser::parse(input, &catalog.plain_wrappers());

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
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
pub struct ParsedCommand {
    pub config: Config,
    /// Pre-target tokens with no place in a `Config` — a foreign wrapper
    /// (`strangle`), its flags, `PROTONPATH=` in Steam mode. Reported so an
    /// import never silently loses part of the command.
    pub dropped: Vec<String>,
}

/// Steam's per-game launch options and compat-tool mapping, freshly read.
#[derive(Clone, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, export_to = "../../src/lib/generated/"))]
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
    let paths = state.store.locked().paths.clone();
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
    if let (Some(cfg), Some(d)) = (&fresh, state.discovery.locked().as_mut()) {
        d.launch_options = cfg.launch_options.clone();
        d.compat_tools = cfg.compat_tools.clone();
    }
    Ok(fresh)
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
    match state.recipes.recipes.get(index) {
        Some(recipe) => apply_recipe_with(&state.catalog, recipe, config),
        None => config,
    }
}

/// [`apply_recipe`]'s logic, without the Tauri state — what the tests drive.
fn apply_recipe_with(catalog: &Catalog, recipe: &recipes::Recipe, config: Config) -> Config {
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

/// Conflict / footgun notices for the current config. `app_id` is the selected
/// game, whose folder is checked for a manual OptiScaler install that would
/// stack under Proton's injected one — a few `read_dir`s, off the UI thread,
/// and only while injection is on.
async fn lint_notices(
    state: &AppState,
    config: &Config,
    app_id: Option<u32>,
) -> Result<Vec<lint::Notice>, String> {
    let value = |key: &str| config.env.iter().find(|(k, _)| k == key).map(|(_, v)| v.trim());
    let injected = value("PROTON_USE_OPTISCALER").is_some_and(|v| v != "0");
    let dir = app_id.filter(|_| injected).and_then(|id| state.game(id)).and_then(|g| g.install_dir);
    let proxy = value("PROTON_OPTISCALER_NAME").filter(|v| !v.is_empty()).unwrap_or("dxgi.dll").to_string();
    let game_files = match dir {
        Some(dir) => tauri::async_runtime::spawn_blocking(move || {
            optiscaler_upgrade::manual_install_files(std::path::Path::new(&dir), &proxy)
        })
        .await
        .map_err(|e| e.to_string())?,
        None => Vec::new(),
    };

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
        &state.store.locked().gpu_gen,
        state.hardware.gpu_gen_detected.as_deref(),
        state.hardware.amd,
    );
    let mut notices =
        lint::warnings(&state.catalog, &options, &state.hardware, &gpu_gen, &game_files);
    notices.extend(lint::invalid_custom_env(&compose::invalid_extra_env(&config.extra_env)));
    // A PATH lookup per enabled `requires` row — a handful of stats, cheap
    // enough for the debounced edit path, and live rather than the scan-time
    // `requires_status` (installing the package mid-session clears it).
    let bins = state.bins();
    notices.extend(lint::missing_programs(&state.catalog, &options, |name| {
        let program = bins.pairs().iter().find(|(n, _)| *n == name).map_or(bins.program(name), |(_, p)| *p);
        crate::which::is_installed(program)
    }));
    Ok(notices)
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

/// Whether the game's anti-cheat runs on Linux, per AreWeAntiCheatYet
/// (see [`crate::anticheat`]). `None` when the list has no anti-cheat entry
/// for it. Opt-in: the frontend only asks when the Settings toggle is on.
#[tauri::command]
pub async fn anticheat_lookup(
    state: State<'_, AppState>,
    app_id: u32,
) -> Result<Option<crate::anticheat::AntiCheat>, String> {
    let Some(game) = state.game(app_id) else {
        return Ok(None);
    };
    tauri::async_runtime::spawn_blocking(move || {
        crate::anticheat::db().map(|db| db.lookup(&game.source, game.app_id, &game.name))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Resolve a game's artwork to an image file (local cache → optional CDN),
/// or `null` if none is available. Runs off the UI thread.
///
/// Returns an opaque key for the file, registered in `art_files`; the webview
/// loads `art://localhost/<key>`, which the asynchronous `art` protocol
/// (`lib.rs`) serves off the UI thread with a long cache lifetime. The key
/// carries the file's mtime, so changed art gets a new URL.
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
    let (src, knd) = (source.clone(), kind.clone());
    let path = tauri::async_runtime::spawn_blocking(move || {
        art::fetch(steam_root, app_id, &src, &knd, online, art_hint)
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(path) = path else { return Ok(None) };
    let key = art::key_for(app_id, &source, &kind, &path);
    state.art_files.locked().insert(key.clone(), path);
    Ok(Some(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steam_game(app_id: u32, source: GameSource) -> games::Game {
        games::Game {
            app_id,
            name: "G".into(),
            source,
            executable: None,
            installed: true,
            heroic_id: None,
            install_dir: None,
            art_url: None,
            nexus: None,
        }
    }

    #[test]
    fn parse_command_reports_only_what_it_could_not_import() {
        let cat = Catalog::bundled();
        let p = parse_command_with(&cat, "PROTON_ENABLE_WAYLAND=1 MY_VAR=\"a b\" A=x;y strangle 60 mangohud %command% -dx11");
        // Catalog env and wrappers come back as rows; unknown env is kept as custom env.
        assert!(p.config.env.iter().any(|(k, v)| k == "PROTON_ENABLE_WAYLAND" && v == "1"));
        assert!(p.config.wrappers.iter().any(|(k, _)| k == "mangohud"));
        assert!(p.config.extra_env.contains("MY_VAR=\"a b\""), "{}", p.config.extra_env);
        assert_eq!(p.config.game_args, "-dx11");
        // `A=x;y` was imported (as env), so it isn't reported dropped; the
        // foreign wrapper and its argument are.
        assert!(p.dropped.contains(&"strangle".to_string()), "{:?}", p.dropped);
        assert!(!p.dropped.iter().any(|t| t.starts_with("A=")), "{:?}", p.dropped);
    }

    #[test]
    fn applying_a_recipe_keeps_env_the_catalog_no_longer_knows() {
        // #62: a stale key used to be erased by the options round trip.
        let cat = Catalog::bundled();
        let recipes = recipes::Recipes::bundled();
        let recipe = &recipes.recipes[0];
        let cfg = Config {
            env: vec![("PROTON_SOMETHING_RETIRED".into(), "1".into())],
            ..Config::default()
        };
        let out = apply_recipe_with(&cat, recipe, cfg);
        assert!(out.extra_env.contains("PROTON_SOMETHING_RETIRED=1"), "{}", out.extra_env);
    }

    #[test]
    fn stale_banner_only_for_a_newer_installed_cachyos() {
        let mut cat = Catalog::bundled();
        cat.meta.proton_cachyos_build = Some("20261005".into());
        let rt = |name: &str| runtime::Runtime {
            internal_name: name.into(),
            display_name: name.into(),
            kind: RuntimeKind::System,
            path: format!("/x/{name}").into(),
        };
        let newer = compute_stale(&cat, &[rt("proton-cachyos-11.0-20261101 (steam linux runtime)")]).unwrap();
        assert_eq!((newer.installed.as_str(), newer.catalog.as_str()), ("20261101", "20261005"));
        assert!(compute_stale(&cat, &[rt("proton-cachyos-11.0-20260901")]).is_none());
        assert!(compute_stale(&cat, &[rt("GE-Proton11-7")]).is_none());
    }

    #[test]
    fn play_stats_come_from_each_source_own_record() {
        let mut cfgs = HashMap::new();
        cfgs.insert(10, steamcfg::AppUserCfg { launch_options: String::new(), last_played: Some(100), playtime_minutes: Some(5) });
        let mut heroic = HashMap::new();
        heroic.insert("h1".to_string(), heroic::PlayStats { last_played: Some(300), playtime_minutes: Some(7) });

        let steam = game_dto(steam_game(10, GameSource::Steam), &cfgs, &heroic);
        assert_eq!((steam.last_played, steam.playtime_minutes), (Some(100), Some(5)));

        let mut h = steam_game(11, GameSource::Heroic);
        h.heroic_id = Some("h1".into());
        let h = game_dto(h, &cfgs, &heroic);
        assert_eq!((h.last_played, h.playtime_minutes), (Some(300), Some(7)));

        // Nexus: its own sessions plus the Heroic sideload's, newest timestamp.
        let mut n = steam_game(12, GameSource::Nexus);
        n.nexus = Some(crate::nexus::NexusInfo {
            slug: "n".into(),
            heroic_app_name: Some("h1".into()),
            last_played: Some(200),
            playtime_minutes: Some(10),
            ..Default::default()
        });
        let n = game_dto(n, &cfgs, &heroic);
        assert_eq!((n.last_played, n.playtime_minutes), (Some(300), Some(17)));
        assert_eq!(n.nexus_slug.as_deref(), Some("n"));

        let sc = game_dto(steam_game(13, GameSource::NonSteam), &cfgs, &heroic);
        assert_eq!((sc.last_played, sc.playtime_minutes), (None, None));
        assert!(sc.nexus_slug.is_none());
    }

    #[test]
    fn a_poisoned_mutex_still_hands_out_its_data() {
        let m = Arc::new(Mutex::new(7));
        let m2 = Arc::clone(&m);
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("poison it");
        })
        .join();
        assert!(m.is_poisoned());
        *m.locked() += 1;
        assert_eq!(*m.locked(), 8);
    }

}

/// Read one of the selected game's logs for the diagnostics viewer: the one
/// `source_id` names, else the newest present (see [`crate::logs`] for where
/// it looks — Proton's log per `PROTON_LOG_DIR` and umu game id, Nexus's
/// launch log, DXVK/VKD3D). `config` is the game's current config, which is
/// where those paths come from. Read-only and off the UI thread; a missing
/// file is a normal `present: false` result, not an error.
#[tauri::command]
pub async fn read_proton_log(
    state: State<'_, AppState>,
    app_id: u32,
    config: Option<Config>,
    source_id: Option<String>,
) -> Result<ProtonLog, String> {
    let game = state.game(app_id);
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).ok_or("$HOME is not set")?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::logs::read(&home, app_id, game.as_ref(), &config.unwrap_or_default(), source_id.as_deref())
    })
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
        let s = state.store.locked();
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
        let s = state.store.locked();
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
    let endpoint = state.store.locked().llm_endpoint.clone();
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
    *state.store.locked() = store;
    let current = Arc::clone(&state.store);
    let save_lock = Arc::clone(&state.save_lock);
    tauri::async_runtime::spawn_blocking(move || {
        let _serial = save_lock.locked();
        // Snapshot under the lock, not before it: a save that lost the race
        // then writes the newest store instead of rolling the file back.
        let snapshot = current.locked().clone();
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

/// lsfg-vk (Lossless Scaling frame generation) on this machine: layer, profiles
/// and `Lossless.dll` — read fresh on every call, because the user edits
/// profiles in `lsfg-vk-ui` while protongen is open. Read-only.
#[tauri::command]
pub async fn lsfg_status(state: State<'_, AppState>) -> Result<lsfg::LsfgStatus, String> {
    let install = state.game(lsfg::LOSSLESS_SCALING_APPID).and_then(|g| g.install_dir);
    tauri::async_runtime::spawn_blocking(move || {
        lsfg::detect(install.as_deref().map(std::path::Path::new))
    })
    .await
    .map_err(|e| e.to_string())
}

/// Check the latest upstream OptiScaler release on `channel` (off the UI
/// thread). Global — not per-game — so the frontend can show "latest: vX.Y.Z"
/// without a game selected. Errors surface directly to the caller; there's no
/// banner to keep quiet for, unlike `check_for_update`.
#[tauri::command]
pub async fn optiscaler_latest(
    channel: optiscaler_upgrade::Channel,
) -> Result<optiscaler_upgrade::OptiscalerRelease, String> {
    tauri::async_runtime::spawn_blocking(move || optiscaler_upgrade::check_latest(channel))
        .await
        .map_err(|e| e.to_string())?
}

/// Download the latest OptiScaler release on `channel` and extract it into `app_id`'s
/// install directory (off the UI thread). The one command in this file that
/// writes into a game's own folder — see `optiscaler_upgrade`'s doc comment.
#[tauri::command]
pub async fn optiscaler_fetch(
    state: State<'_, AppState>,
    app_id: u32,
    channel: optiscaler_upgrade::Channel,
) -> Result<optiscaler_upgrade::OptiscalerExtractResult, String> {
    let dir = state
        .game(app_id)
        .and_then(|g| g.install_dir)
        .ok_or_else(|| "no install directory known for this game".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        optiscaler_upgrade::fetch_and_extract(std::path::Path::new(&dir), channel)
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

/// Check whether a newer GE-Proton / proton-cachyos build than the newest one
/// installed has been released (off the UI thread). Uses the runtimes from the
/// last discovery pass, so it must run after `bootstrap`. Read-only: it only
/// reports, the user installs. Never errors; a failed fetch just reports less.
#[tauri::command]
pub async fn check_runtime_updates(
    state: State<'_, AppState>,
) -> Result<Vec<runtime_updates::RuntimeUpdate>, String> {
    let runtimes: Vec<RuntimeDto> = state
        .discovery
        .locked()
        .as_ref()
        .map(|d| d.runtimes.clone())
        .unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        let installed: Vec<runtime_updates::Installed> = runtimes
            .iter()
            .map(|r| runtime_updates::Installed { name: &r.display_name, kind: &r.kind })
            .collect();
        runtime_updates::check_blocking(&installed)
    })
    .await
    .map_err(|e| e.to_string())
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
