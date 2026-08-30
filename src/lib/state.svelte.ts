import { ipc } from "./ipc";
import { toast } from "./toast.svelte";
import { history } from "./history.svelte";
import type { Entry, Snapshot } from "./history.svelte";
import { applyTheme, DEFAULT_THEME } from "./themes";
import { irrelevance, isRecommended, mergeIntoExtraEnv, splitExtraEnv, tokenizeEnv } from "./util";
import { emptyConfig, isAdvanced } from "./types";
import type {
  Catalog,
  Config,
  DiffStatus,
  GameDto,
  GpuGen,
  Hardware,
  HwCaps,
  Notice,
  LaunchDiff,
  LlmSuggestion,
  OptiscalerExtractResult,
  OptiscalerRelease,
  OptiscalerStatus,
  Recipe,
  RuntimeDto,
  ConfigWarning,
  StaleInfo,
  Store,
  SyncState,
  Tier,
  Token,
  TroubleshootResult,
  UiMode,
  UpdateInfo,
} from "./types";

interface OptState {
  enabled: boolean;
  value: string;
}

const EMPTY_CATALOG: Catalog = {
  meta: { proton_cachyos_build: null, updated: null },
  wrappers: [],
  envs: [],
};

const EMPTY_STORE: Store = {
  theme: DEFAULT_THEME,
  presets: [],
  game_memory: {},
  dismissed_cachyos_build: "",
  dismissed_update_version: "",
  show_irrelevant: false,
  show_advanced: false,
  hdr: false,
  fsr4: false,
  gpu_gen: "",
  protondb_auto: false,
  llm_enabled: false,
  llm_endpoint: "http://127.0.0.1:1234/v1",
  llm_model: "gpt-oss-20b",
  favorites: [],
  library_sort: "",
  last_session: null,
  last_game_appid: null,
  ui_mode: "simple",
  seen_intro_tour: false,
  paths: { steam_roots: [], steam_libraries: [], proton_dirs: [], bins: {} },
  global_profile: null,
};

/** Library sort ids. Kept here because both the toolbar and the comparator in
 *  `Library.svelte` need to agree on them, and the persisted value is a bare
 *  string that may predate any of them. */
export type LibrarySort = "recent" | "alpha" | "tuned";
export const DEFAULT_LIBRARY_SORT: LibrarySort = "recent";

export type ArtKind = "portrait" | "hero" | "header";

/** Simultaneous art lookups. Bounded because the grid now requests art for
 *  every tile that scrolls near the viewport instead of a fixed first-N. */
const ART_CONCURRENCY = 12;

/** Single source of truth for the whole UI, backed by Svelte 5 runes. */
class AppStore {
  // ---- bootstrap data (mostly immutable) ----
  ready = $state(false);
  loadError = $state<string | null>(null);
  steamRoot = $state<string | null>(null);
  catalog = $state<Catalog>(EMPTY_CATALOG);
  categories = $state<string[]>([]);
  recipes = $state<Recipe[]>([]);
  runtimes = $state<RuntimeDto[]>([]);
  games = $state<GameDto[]>([]);
  hardware = $state<Hardware>({
    nvidia: false,
    amd: false,
    intel: false,
    wayland: false,
    kde: false,
    ntsync: false,
    distro: "",
    kernel: "",
    ram_gb: 0,
    cpu_model: "",
  });
  requiresStatus = $state<Record<string, boolean>>({});
  launchOptions = $state<Record<string, string>>({});
  /** appid -> whether Steam already has the remembered command. Only games with
   *  a remembered config appear; the grid shows nothing for the rest. */
  launchStatuses = $state<Record<string, DiffStatus>>({});
  compatTools = $state<Record<string, string>>({});
  stale = $state<StaleInfo | null>(null);
  /** User params.toml / recipes.toml overrides that failed to parse. */
  configWarnings = $state<ConfigWarning[]>([]);
  update = $state<UpdateInfo | null>(null);
  updating = $state(false);
  /** True while a library re-scan (rescan IPC) is in flight. */
  refreshing = $state(false);
  store = $state<Store>(EMPTY_STORE);

  // ---- builder selection ----
  umu = $state(false);
  selectedRuntime = $state<RuntimeDto | null>(null);
  env = $state<Record<string, OptState>>({});
  wrap = $state<Record<string, OptState>>({});
  extraEnv = $state("");
  gameArgs = $state("");
  umuExe = $state("");
  umuWineprefix = $state("");
  umuGameid = $state("");
  selectedAppId = $state<number | null>(null);
  selectedGameName = $state<string | null>(null);

  // ---- derived/live ----
  command = $state("");
  notices = $state<Notice[]>([]);
  /** The command split into coloured/annotatable pieces. Concatenating every
   *  `text` reproduces `command` byte-for-byte — never re-join with spaces. */
  tokens = $state<Token[]>([]);
  /** Semantic comparison against Steam's current launch options. Null when
   *  there is nothing to compare (see `currentLaunchOptions`). */
  launchDiff = $state<LaunchDiff | null>(null);
  /** Briefly true right after the session is written to disk (trust cue). */
  saved = $state(false);

  // ---- ephemeral UI ----
  /** Top-level screen: the cover-art library grid, or the focused builder. The
   *  app opens on the library; picking a game (or "generic") enters the builder. */
  view = $state<"library" | "builder">("library");
  activePresetName = $state<string | null>(null);
  /** Console layout: which section the main panel shows. "recipes" | "game" |
   *  "Wrappers" | a parameter category name. */
  activeSection = $state<string>("recipes");
  /** Global parameter search; when non-empty the main panel shows flat results. */
  paramQuery = $state("");
  /** Overlay visibility. These live here rather than in Header.svelte so the
   *  command palette and the Ctrl+, binding can open them from anywhere. */
  showImport = $state(false);
  showSave = $state(false);
  showSettings = $state(false);
  showPalette = $state(false);
  showShortcuts = $state(false);
  /** The per-game Proton log viewer (opened from the header, for the selected
   *  game). Lives here so the palette can open it too. */
  showLogs = $state(false);
  // ---- local-LLM log coach (opt-in; driven from the log viewer) ----
  aiLoading = $state(false);
  aiResult = $state<LlmSuggestion | null>(null);
  aiError = $state<string | null>(null);
  // ---- local-LLM symptom troubleshooter (opt-in; its own header dialog) ----
  showTroubleshooter = $state(false);
  tsLoading = $state(false);
  tsResult = $state<TroubleshootResult | null>(null);
  tsError = $state<string | null>(null);
  /** True while the "apply your default profile?" prompt is up for a
   *  freshly-opened game that had no saved config. Ephemeral, never persisted. */
  pendingDefaultPrompt = $state(false);

  // ---- game art (lazy, cached): key `${source}:${appId}:${kind}` ----
  //   undefined = not requested/loading · null = none found · string = data URL
  artCache = $state<Record<string, string | null>>({});
  private artRequested = new Set<string>();
  /** Art fetches currently awaiting IPC, and the backlog behind them. Each call
   *  is a Tauri round-trip returning a base64 data URL, so scrolling fast through
   *  a few thousand tiles would otherwise fan out into thousands of concurrent
   *  requests. */
  private artInFlight = 0;
  private artQueue: Array<() => void> = [];

  /** Last build_command failure, rendered inline in the command bar. */
  buildError = $state<string | null>(null);
  /** init() failure; the app shows an error screen with Retry instead of spinning. */
  initError = $state<string | null>(null);
  /** Last settings-write failure, shown as a sticky banner until a save works. */
  persistError = $state<string | null>(null);
  /** Rate-limits the persist-failure toast; the banner carries the detail. */
  private lastPersistToast = 0;

  /** Monotonic guard so a slow earlier recompute cannot overwrite a newer one. */
  private recomputeSeq = 0;
  /** $effect.root must be registered exactly once, even across Retry. */
  private effectsRegistered = false;

  private recomputeTimer: ReturnType<typeof setTimeout> | null = null;
  private sessionTimer: ReturnType<typeof setTimeout> | null = null;
  private savedTimer: ReturnType<typeof setTimeout> | null = null;
  /** The first persist just writes the restored session back; don't flash "Saved". */
  private firstPersist = true;

  async init() {
    this.initError = null;
    try {
      await this.load();
    } catch (e) {
      // Leave ready false so App renders the error screen rather than a
      // spinner that never resolves.
      this.initError = String(e);
      return;
    }
    this.startReactivity();
  }

  private async load() {
    const b = await ipc.bootstrap();
    this.loadError = b.load_error;
    this.steamRoot = b.steam_root;
    this.catalog = b.catalog;
    this.categories = b.categories;
    this.recipes = b.recipes;
    this.runtimes = this.withAutoRuntime(b.runtimes);
    this.games = b.games;
    this.hardware = b.hardware;
    this.requiresStatus = b.requires_status;
    this.launchOptions = b.launch_options;
    this.compatTools = b.compat_tools;
    this.stale = b.stale;
    this.configWarnings = b.config_warnings;
    this.store = b.store;

    applyTheme(b.store.theme || DEFAULT_THEME);

    // Establish the default runtime first; a restored session overrides it.
    this.selectedRuntime = this.defaultRuntime();

    // Restore the last session (selected game + every builder selection) so the
    // user reopens exactly where they left off; otherwise start from defaults.
    const sess = this.store.last_session;
    if (sess) {
      if (this.store.last_game_appid != null) {
        const g = this.games.find((x) => x.app_id === this.store.last_game_appid);
        if (g) {
          this.selectedAppId = g.app_id;
          this.selectedGameName = g.name;
        }
      }
      this.loadConfig(sess);
    } else {
      this.resetOptions();
    }

    // Seed the undo baseline with the state the user is actually looking at, so
    // restoring a session isn't itself the first undo entry.
    history.reset(this.snapshot());

    this.ready = true;

    // Badge the library grid. After `ready` so the first paint isn't waiting on
    // it — the grid renders unbadged and fills in.
    this.refreshLaunchStatuses();

    // Check for a newer release in the background; never blocks launch.
    this.checkForUpdate();
  }

  /**
   * Recompute the command + lint, and persist the session, whenever any
   * builder input changes. Called after a successful load so the first effect
   * run captures the restored state rather than defaults.
   *
   * Guarded because Retry calls init() again: a second $effect.root would
   * leave two live roots, double-recomputing and double-persisting for the
   * rest of the session.
   */
  private startReactivity() {
    if (this.effectsRegistered) return;
    this.effectsRegistered = true;

    $effect.root(() => {
      $effect(() => {
        const cfg = this.toConfig();
        // Read so that a rescan — which rewrites launchOptions and games — also
        // re-runs the sync diff. Otherwise the pill would keep asserting a
        // verdict from before the rescan.
        void this.currentLaunchOptions;
        // Every mutation path funnels through here, so history cannot miss one.
        // The extra reads (appId/gameName/preset) are what make a game switch
        // undoable — toConfig() alone wouldn't see it.
        history.observe({
          config: cfg,
          appId: this.selectedAppId,
          gameName: this.selectedGameName,
          activePresetName: this.activePresetName,
        });
        this.scheduleRecompute(cfg);
        this.scheduleSessionPersist();
      });
    });
  }

  /** Prepend the synthetic GE-Proton auto-download entry to a discovered runtime
   *  list. umu resolves the "GE-Proton" codename and auto-downloads the latest;
   *  its path IS the codename. Only meaningful in umu mode (Steam ignores
   *  PROTONPATH). */
  private withAutoRuntime(runtimes: RuntimeDto[]): RuntimeDto[] {
    return [
      {
        internal_name: "GE-Proton",
        display_name: "GE-Proton (latest · umu auto-download)",
        kind: "auto",
        path: "GE-Proton",
      },
      ...runtimes,
    ];
  }

  /**
   * Re-scan the library (games, runtimes, shortcuts) without restarting. Only
   * the discovery-derived fields are replaced; builder selections, the store and
   * the theme are left intact. Current selections are re-validated against the
   * fresh lists so a removed game/runtime falls back gracefully.
   *
   * Reports what happened, mirroring `checkForUpdate()`: `rescan` now runs off
   * the backend's main thread, so it signals failure by *rejecting* rather than
   * by returning a Bootstrap carrying `load_error`. A caller that announced a
   * refresh needs to know, or it toasts "Library refreshed" over a failure.
   */
  async refresh(): Promise<"ok" | "busy" | "failed"> {
    if (this.refreshing) return "busy";
    this.refreshing = true;
    try {
      const b = await ipc.rescan();
      this.loadError = b.load_error;
      this.steamRoot = b.steam_root;
      this.runtimes = this.withAutoRuntime(b.runtimes);
      this.games = b.games;
      this.launchOptions = b.launch_options;
      this.compatTools = b.compat_tools;
      this.stale = b.stale;
      // Both are recomputed by `rescan` and must be copied, or a corrected
      // Settings path would never clear its banner and a fixed binary override
      // would never turn its badge green.
      this.configWarnings = b.config_warnings;
      this.requiresStatus = b.requires_status;

      // Re-validate current selections against the refreshed lists.
      if (
        this.selectedRuntime &&
        !this.runtimes.some((r) => r.path === this.selectedRuntime!.path)
      ) {
        this.selectedRuntime = this.defaultRuntime();
      }
      if (
        this.selectedAppId != null &&
        !this.games.some((g) => g.app_id === this.selectedAppId)
      ) {
        this.selectedAppId = null;
        this.selectedGameName = null;
      }
    } catch (e) {
      // Caught rather than rethrown: `scheduleRescan` fires this on a debounce
      // with no caller to catch it, and an unhandled rejection there is
      // invisible. The banner is the durable signal.
      console.error("rescan failed", e);
      this.loadError = String(e);
      return "failed";
    } finally {
      this.refreshing = false;
    }
    // A refresh is the user asking for a fresh look, so give art that came back
    // empty another chance rather than leaving those tiles blank all session.
    this.retryFailedArt();
    // launchOptions just changed, so every badge is potentially stale.
    this.refreshLaunchStatuses();
    return "ok";
  }

  /** Recompute the per-game applied/drifted badges for the library grid.
   *
   *  Deliberately *not* wired into `persistStore`: that fires on a 500 ms
   *  debounce while the user types in the builder, when the grid isn't even on
   *  screen. Keeping `save_store` fire-and-forget avoids entangling persistence
   *  with status. The three call sites — startup, library refresh, and
   *  returning to the grid — are the moments the badges are about to be seen. */
  async refreshLaunchStatuses() {
    try {
      this.launchStatuses = await ipc.launchStatuses(
        $state.snapshot(this.store.game_memory),
        $state.snapshot(this.launchOptions),
      );
    } catch (e) {
      console.error("launchStatuses failed", e);
    }
  }

  /** Preferred default runtime: an installed proton-cachyos, else the first
   *  real (non-synthetic) runtime, else the GE-Proton-auto entry. */
  private defaultRuntime(): RuntimeDto | null {
    return (
      this.runtimes.find((r) => r.display_name.toLowerCase().includes("cachyos")) ??
      this.runtimes.find((r) => r.kind !== "auto") ??
      this.runtimes[0] ??
      null
    );
  }

  // ----------------------------- option helpers -----------------------------

  resetOptions() {
    const env: Record<string, OptState> = {};
    for (const e of this.catalog.envs) {
      env[e.key] = { enabled: false, value: e.default_value };
    }
    const wrap: Record<string, OptState> = {};
    for (const w of this.catalog.wrappers) {
      wrap[w.key] = { enabled: false, value: w.default_value };
    }
    this.env = env;
    this.wrap = wrap;
    this.extraEnv = "";
    this.gameArgs = "";
    // Attribution describes how the *current* values were reached, so it cannot
    // outlive them. This also covers undo and preset/game loads, which all funnel
    // through loadConfig → resetOptions.
    this.recipeOrigin = {};
  }

  toggleEnv(key: string) {
    const s = this.env[key];
    if (!s) return;
    s.enabled = !s.enabled;
    this.disownParam(key);
    this.mark(`${s.enabled ? "enable" : "disable"} ${key}`);
  }
  setEnvValue(key: string, value: string) {
    const s = this.env[key];
    if (!s) return;
    s.value = value;
    this.disownParam(key);
    // Typing coalesces into one entry; note() just gives it a real name.
    history.note(`set ${key}`);
  }
  toggleWrap(key: string) {
    const s = this.wrap[key];
    if (!s) return;
    s.enabled = !s.enabled;
    this.disownParam(key);
    this.mark(`${s.enabled ? "enable" : "disable"} ${key}`);
  }
  setWrapValue(key: string, value: string) {
    const s = this.wrap[key];
    if (!s) return;
    s.value = value;
    this.disownParam(key);
    history.note(`set ${key}`);
  }

  /**
   * Which recipe set each parameter, so a row can say where its value came from.
   * Apply two recipes and there is otherwise no way to attribute any setting.
   *
   * Not persisted: it describes how the current state was *reached*, which stops
   * being true the moment a remembered config is loaded from disk.
   */
  recipeOrigin = $state<Record<string, string>>({});

  /** Editing a row by hand makes the attribution false, so drop it. */
  private disownParam(key: string) {
    if (this.recipeOrigin[key]) delete this.recipeOrigin[key];
  }

  /** Steam vs umu mode. A setter rather than a bare assignment from the toggle
   *  so the history entry gets a label. */
  setUmu(v: boolean) {
    if (this.umu === v) return;
    this.umu = v;
    this.mark(v ? "switch to umu mode" : "switch to Steam mode");
  }

  setRuntime(r: RuntimeDto) {
    if (this.selectedRuntime?.path === r.path) return;
    this.selectedRuntime = r;
    this.mark(`select ${r.display_name}`);
  }

  /** Everything the user has turned on, for the nav badge and the Active view. */
  get activeCount(): number {
    const envs = this.catalog.envs.filter((e) => this.env[e.key]?.enabled).length;
    const wraps = this.catalog.wrappers.filter((w) => this.wrap[w.key]?.enabled).length;
    return envs + wraps + splitExtraEnv(this.extraEnv).length;
  }

  /** Drop one `K=V` token from the custom-env string, keeping the rest verbatim. */
  removeExtraEnv(raw: string) {
    const kept = tokenizeEnv(this.extraEnv).filter((t) => t !== raw);
    this.extraEnv = kept.join(" ");
    this.mark(`remove ${raw.split("=")[0]}`);
  }

  enabledCountInCategory(category: string): number {
    return this.catalog.envs.filter(
      (e) => e.category === category && this.env[e.key]?.enabled,
    ).length;
  }

  /**
   * Categories with at least one entry this machine can actually use.
   *
   * The nav rail listed every category unconditionally, so a section whose
   * every parameter was filtered out still got a row — clicking it landed on an
   * empty panel. On an AMD box that is the whole NVIDIA section: thirteen
   * `gpu = "nvidia"` entries, none of them rendered, under a heading promising
   * NVIDIA options.
   *
   * Filtered on hardware relevance only, deliberately not on `tier`: a section
   * that is entirely advanced still deserves its row, because the panel there
   * shows a "N advanced hidden · Show advanced" affordance. Hiding it would make
   * those parameters unreachable by browsing. `show_irrelevant` restores every
   * row, so nothing is permanently out of reach either way.
   */
  get visibleCategories(): string[] {
    if (this.store.show_irrelevant) return this.categories;
    return this.categories.filter((c) =>
      this.catalog.envs.some(
        (e) => e.category === c && !irrelevance(this.hwCaps, e.gpu, e.needs),
      ),
    );
  }

  // ------------------------------- config I/O -------------------------------

  toConfig(): Config {
    const env = this.catalog.envs
      .filter((e) => this.env[e.key]?.enabled)
      .map((e) => [e.key, this.env[e.key].value] as [string, string]);
    const wrappers = this.catalog.wrappers
      .filter((w) => this.wrap[w.key]?.enabled)
      .map((w) => [w.key, this.wrap[w.key].value] as [string, string]);
    return {
      umu: this.umu,
      runtime: this.selectedRuntime?.internal_name ?? null,
      env,
      wrappers,
      extra_env: this.extraEnv,
      umu_exe: this.umuExe,
      umu_wineprefix: this.umuWineprefix,
      umu_gameid: this.umuGameid,
      game_args: this.gameArgs,
    };
  }

  loadConfig(cfg: Config) {
    this.resetOptions();
    // Mirror of `store::options_from_lists`: an env key the catalog no longer
    // has is re-homed into the custom-env field rather than dropped. Dropping it
    // was not merely cosmetic — `toConfig()` rebuilds `env` by walking
    // `this.catalog.envs`, so the key was erased from the preset or game_memory
    // entry the moment anything re-saved (#62).
    const leftover: [string, string][] = [];
    for (const [k, v] of cfg.env) {
      if (this.env[k]) this.env[k] = { enabled: true, value: v };
      else leftover.push([k, v]);
    }
    // Wrapper keys stay dropped, as on the Rust side: a wrapper is a program
    // token from a closed enum, so there is nothing to re-home it into.
    for (const [k, v] of cfg.wrappers) {
      if (this.wrap[k]) this.wrap[k] = { enabled: true, value: v };
    }
    this.umu = cfg.umu;
    // Idempotent with the backend merge: after a round-trip the key is already
    // in `cfg.extra_env`, so it produces no leftover and nothing is duplicated.
    this.extraEnv = mergeIntoExtraEnv(cfg.extra_env, leftover);
    this.gameArgs = cfg.game_args;
    this.umuExe = cfg.umu_exe;
    this.umuWineprefix = cfg.umu_wineprefix;
    this.umuGameid = cfg.umu_gameid;
    if (cfg.runtime) {
      const r = this.runtimes.find((x) => x.internal_name === cfg.runtime);
      if (r) this.selectedRuntime = r;
    }
  }

  /** Reset the command back to defaults, keeping the selected game. Just another
   *  undoable action now — the old "returns the prior config so the caller can
   *  offer a 2-second undo" contract is gone, replaced by the real stack. */
  resetCommand() {
    this.resetOptions();
    this.umu = false;
    this.umuExe = "";
    this.umuWineprefix = "";
    this.umuGameid = "";
    this.activePresetName = null;
    this.selectedRuntime = this.defaultRuntime();
    this.mark("reset command");
  }

  // -------------------------------- history ---------------------------------

  /** Everything undo has to restore, as history sees it. */
  private snapshot(): Snapshot {
    return {
      config: this.toConfig(),
      appId: this.selectedAppId,
      gameName: this.selectedGameName,
      activePresetName: this.activePresetName,
    };
  }

  /** Land a history entry now, labelled, instead of waiting out the coalescing
   *  timer. Call at the *end* of a discrete mutator, once state has settled. */
  private mark(label: string) {
    history.flush(label, this.snapshot());
  }

  /** Name the coalescing burst a free-text field is producing, for fields bound
   *  directly with `bind:value` (there is no mutator to label it from). Purely
   *  cosmetic: the entry lands either way, this just stops it reading "edit". */
  noteEdit(label: string) {
    history.note(label);
  }

  undo() {
    const e = history.undo();
    if (!e) return;
    this.applyEntry(e);
    toast.info(`Undid: ${e.label}`);
  }

  redo() {
    const e = history.redo();
    if (!e) return;
    this.applyEntry(e);
    toast.info(`Redid: ${e.label}`);
  }

  /** Restore a history entry. Sets the game fields *directly* rather than going
   *  through selectGame(), which would persist and then re-read that game's
   *  memory — undoing a game switch would land on the memory instead of the
   *  state we recorded. */
  private applyEntry(e: Entry) {
    this.selectedAppId = e.appId;
    this.selectedGameName = e.gameName;
    this.loadConfig(e.config);
    this.activePresetName = e.activePresetName;
  }

  private protonPath(): string | null {
    return this.selectedRuntime?.path ?? null;
  }

  private scheduleRecompute(cfg: Config) {
    if (this.recomputeTimer) clearTimeout(this.recomputeTimer);
    const seq = ++this.recomputeSeq;
    this.recomputeTimer = setTimeout(async () => {
      const path = this.protonPath();

      // Separate try/catch per call: a lint rejection must not blank an
      // otherwise-valid command, and vice versa.
      let built: string | null = null;
      try {
        const command = await ipc.buildCommand(cfg, path);
        built = command;
        // Two awaits race freely, so a slower earlier invocation can resolve
        // after a newer one. Without this guard it would overwrite the fresh
        // command with a stale value.
        if (seq === this.recomputeSeq) {
          this.command = command;
          this.buildError = null;
        }
      } catch (e) {
        if (seq === this.recomputeSeq) {
          // Surfaced inline in the command bar, never toasted: this fires on
          // every keystroke while broken, and a toast storm would bury it.
          this.buildError = String(e);
        }
      }

      // Tokenize for the coloured preview. Only meaningful if the build worked;
      // on failure the old tokens are left alone, matching how the stale command
      // string stays visible with a warning rather than blanking.
      if (built !== null) {
        try {
          const tokens = await ipc.explainCommand(built);
          if (seq === this.recomputeSeq) this.tokens = tokens;
        } catch (e) {
          console.error("explainCommand failed", e);
          // Fall back to one opaque token so the body still renders the exact
          // command rather than going blank.
          if (seq === this.recomputeSeq) {
            this.tokens = [{ text: built, kind: "unknown", key: null }];
          }
        }
      }

      try {
        const notices = await ipc.lint(cfg);
        if (seq === this.recomputeSeq) this.notices = notices;
      } catch (e) {
        console.error("lint failed", e);
      }

      // Third await in the existing debounce rather than a timer of its own.
      // Skipped entirely when there is nothing to compare against, which is the
      // common case (generic builds, shortcuts, umu).
      const current = this.currentLaunchOptions;
      if (built !== null && current !== null) {
        try {
          const diff = await ipc.launchDiff(built, current);
          if (seq === this.recomputeSeq) this.launchDiff = diff;
        } catch (e) {
          console.error("launchDiff failed", e);
          // Better to show no pill than a stale verdict about whether the user's
          // Steam config matches.
          if (seq === this.recomputeSeq) this.launchDiff = null;
        }
      } else if (seq === this.recomputeSeq) {
        this.launchDiff = null;
      }
    }, 60);
  }

  /** Re-run the build immediately, for the inline error's Retry. */
  retryBuild() {
    this.scheduleRecompute(this.toConfig());
  }

  /** Persist the current builder state as the "last session" (and keep the
   *  selected game's memory fresh), debounced to keep disk writes cheap. */
  private scheduleSessionPersist() {
    if (!this.ready) return;
    if (this.sessionTimer) clearTimeout(this.sessionTimer);
    this.sessionTimer = setTimeout(() => {
      const cfg = this.toConfig();
      this.store.last_session = cfg;
      this.store.last_game_appid = this.selectedAppId;
      if (this.selectedAppId != null) {
        this.store.game_memory[String(this.selectedAppId)] = cfg;
      }
      this.persistStore();
      if (this.firstPersist) this.firstPersist = false;
      else this.flashSaved();
    }, 500);
  }

  /** Briefly surface a "Saved" cue after a session write. */
  private flashSaved() {
    this.saved = true;
    if (this.savedTimer) clearTimeout(this.savedTimer);
    this.savedTimer = setTimeout(() => (this.saved = false), 1200);
  }

  // ------------------------------- navigation -------------------------------

  /** Pick a game from the library and drop into the focused builder. */
  openGame(game: GameDto) {
    this.selectGame(game);
    this.view = "builder";
  }

  /** Build a command with no game attached (generic path). */
  openGeneric() {
    this.selectGame(null);
    this.view = "builder";
  }

  /** Return to the cover-art library grid, keeping the current selection. */
  backToLibrary() {
    this.view = "library";
    // Flush the config just edited into memory before badging, the same way
    // `selectGame` does on the way out — the debounced session persist may not
    // have fired yet, and a badge computed from the previous config would be
    // wrong exactly when the user looks at it.
    if (this.selectedAppId != null) {
      this.store.game_memory[String(this.selectedAppId)] = this.toConfig();
    }
    this.refreshLaunchStatuses();
  }

  /** The selected game, resolved against the discovery list. */
  get selectedGame(): GameDto | null {
    if (this.selectedAppId == null) return null;
    return this.games.find((g) => g.app_id === this.selectedAppId) ?? null;
  }

  /** Heroic's per-game id for the selected game, or null when it isn't a Heroic
   *  game — the gate for the "Apply to Heroic" action. */
  get heroicId(): string | null {
    const g = this.selectedGame;
    return g?.source === "heroic" ? (g.heroic_id ?? null) : null;
  }

  /**
   * The appid a `steam://` deep link can address, or null when one would be
   * meaningless — no game, a non-Steam shortcut (whose appid is a synthetic
   * shortcut id that Steam's verbs know nothing about), or no Steam install
   * found at all.
   */
  get steamAppId(): number | null {
    if (this.steamRoot == null) return null;
    const g = this.selectedGame;
    return g && g.source === "steam" ? g.app_id : null;
  }

  // --------------------------- sync with Steam ------------------------------

  /**
   * Steam's current launch options for the selected game, or null when there is
   * nothing to compare against.
   *
   * `steamAppId` carries the hard gate: a non-Steam shortcut returns null, so an
   * absent entry can never be mistaken for "Steam has no launch options". For a
   * real Steam game an absent entry genuinely does mean none are set.
   */
  get currentLaunchOptions(): string | null {
    const id = this.steamAppId;
    if (id == null) return null;
    return this.launchOptions[String(id)] ?? "";
  }

  /** What the pill should say, or "hidden" when it must not appear. */
  get syncState(): SyncState {
    if (this.umu) return "hidden";
    if (this.currentLaunchOptions === null) return "hidden";
    const s = this.launchDiff?.status;
    return s === "in-sync" || s === "drifted" || s === "not-applied" ? s : "hidden";
  }

  /** How many concrete differences there are, for "N changes not pasted". */
  get driftCount(): number {
    const d = this.launchDiff;
    if (!d) return 0;
    return (
      d.added.length +
      d.removed.length +
      d.changed.length +
      d.unmodeled.length +
      (d.game_args ? 1 : 0)
    );
  }

  /**
   * Whether Steam's compat-tool mapping can be meaningfully compared to the
   * selected runtime. `valve` and `auto` runtimes carry placeholder internal
   * names (runtime.rs) that can never match a `config.vdf` mapping, so
   * comparing them would always read as a mismatch.
   */
  get runtimeComparable(): boolean {
    const r = this.selectedRuntime;
    return (
      this.steamAppId != null && r != null && r.kind !== "valve" && r.kind !== "auto"
    );
  }

  /**
   * Steam's Proton dropdown disagrees with the selected runtime. `steam` is ""
   * when Steam has no mapping at all — still a disagreement worth reporting,
   * just phrased as an instruction rather than a correction.
   *
   * Deliberately *not* folded into the drift verdict: the compat tool is a
   * separate Steam control with its own paste target, so folding it in would
   * make "drifted" un-actionable from a library tile (#41).
   */
  get runtimeMismatch(): { steam: string; wanted: string } | null {
    if (!this.runtimeComparable) return null;
    const steam = this.compatTools[String(this.steamAppId)] ?? "";
    const r = this.selectedRuntime!;
    return steam === r.internal_name ? null : { steam, wanted: r.display_name };
  }

  // ------------------------------- game memory ------------------------------

  selectGame(game: GameDto | null) {
    // Persist the outgoing game's config.
    if (this.selectedAppId != null) {
      this.store.game_memory[String(this.selectedAppId)] = this.toConfig();
      this.persistStore();
    }

    // Any pending prompt belonged to the game we're leaving.
    this.pendingDefaultPrompt = false;

    if (!game) {
      this.selectedAppId = null;
      this.selectedGameName = null;
      this.mark("switch to no game");
      return;
    }

    this.selectedAppId = game.app_id;
    this.selectedGameName = game.name;
    this.activePresetName = null;

    if (game.executable) this.umuExe = game.executable;

    const remembered = this.store.game_memory[String(game.app_id)];
    if (remembered) {
      this.loadConfig(remembered);
    } else {
      this.resetOptions();
      // Heroic launches its games itself via umu/Proton; Steam mode's `%command%`
      // is meaningless for them, so default a freshly-opened Heroic game to umu.
      if (game.source === "heroic") this.umu = true;
      // No saved tuning for this game: offer the default profile if the user has
      // authored one. Prompt-each-time rather than auto-apply, so it never
      // silently overwrites what a first-time game should start clean with.
      if (this.store.global_profile) this.pendingDefaultPrompt = true;
    }

    // Undoable on purpose: "I switched games and lost my tuning" is exactly the
    // trust failure the stack exists to fix.
    this.mark(`open ${game.name}`);
  }

  /**
   * Write the current tuning (env vars + wrappers) into the selected Heroic
   * game's per-game config. No-op unless a Heroic game is selected. The backend
   * backs the file up first and preserves everything it doesn't own.
   */
  async injectHeroic() {
    const id = this.heroicId;
    if (id == null) return;
    try {
      await ipc.injectHeroic(id, this.toConfig());
      toast.success("Applied to Heroic — restart Heroic to pick it up (backup saved)", {
        ms: 6000,
      });
    } catch (e) {
      toast.error(`Couldn't write to Heroic: ${e}`, { ms: 6000 });
    }
  }

  /**
   * Merge `pendingMangoSystemConfig` into the real, system-wide MangoHud.conf,
   * so it becomes the default for every MangoHud-enabled program — not just
   * this app's own generated command. The backend backs the file up first and
   * preserves every line it doesn't own (font, keybinds, blacklist, unmodeled
   * colors, comments); a key the new config drops is cleared to match, same as
   * `injectHeroic` writing `false` to fully turn off a wrapper it owns.
   */
  async exportMangoSystemWide() {
    try {
      const res = await ipc.exportMangohudSystem(this.pendingMangoSystemConfig);
      const cleared = res.cleared_keys.length
        ? ` (cleared: ${res.cleared_keys.join(", ")})`
        : "";
      toast.success(`Set as system MangoHud default — backup saved${cleared}`, { ms: 6000 });
    } catch (e) {
      toast.error(`Couldn't write MangoHud.conf: ${e}`, { ms: 6000 });
    }
  }

  // ------------------------------- presets ----------------------------------

  /** Presets saved against the currently selected game (by app id). Empty when
   *  no game is selected or none match — callers should fall back to `otherPresets`. */
  get presetsForCurrentGame() {
    if (this.selectedAppId == null) return [];
    return this.store.presets.filter((p) => p.game_appid === this.selectedAppId);
  }

  /** Every other saved preset: global ones (no game_appid) plus ones saved
   *  against a different game than the current selection. */
  get otherPresets() {
    if (this.selectedAppId == null) return this.store.presets;
    return this.store.presets.filter((p) => p.game_appid !== this.selectedAppId);
  }

  savePreset(name: string) {
    const preset = {
      name,
      game_appid: this.selectedAppId,
      game_name: this.selectedGameName,
      config: this.toConfig(),
    };
    const i = this.store.presets.findIndex((p) => p.name === name);
    if (i >= 0) this.store.presets[i] = preset;
    else this.store.presets.push(preset);
    this.activePresetName = name;
    this.persistStore();
  }

  loadPreset(name: string) {
    const p = this.store.presets.find((x) => x.name === name);
    if (!p) return;
    this.loadConfig(p.config);
    this.activePresetName = name;
    this.mark(`load preset "${name}"`);
  }

  deletePreset(name: string) {
    this.store.presets = this.store.presets.filter((p) => p.name !== name);
    if (this.activePresetName === name) this.activePresetName = null;
    this.persistStore();
  }

  // --------------------------- global profile -------------------------------

  /** Save the current build as the reusable global profile (Settings). */
  setGlobalProfileFromCurrent() {
    this.store.global_profile = this.toConfig();
    this.persistStore();
  }

  clearGlobalProfile() {
    this.store.global_profile = null;
    this.persistStore();
  }

  /** Replace the current selection with the saved global profile. Undoable,
   *  mirroring `loadPreset`. No-op when no profile is set. */
  applyGlobalProfile() {
    const gp = this.store.global_profile;
    if (!gp) return;
    this.loadConfig(gp);
    this.mark("apply global profile");
  }

  // ------------------------------- import -----------------------------------

  async importCommand(text: string) {
    const cfg = await ipc.parseCommand(text);
    this.loadConfig(cfg);
    this.mark("import command");
  }

  // ------------------------------- mangohud ---------------------------------

  applyMango(config: string) {
    if (this.env["MANGOHUD_CONFIG"]) {
      this.env["MANGOHUD_CONFIG"] = { enabled: true, value: config };
    } else {
      // Fall back to custom env if the catalog lacks the key.
      this.extraEnv = `${this.extraEnv} MANGOHUD_CONFIG=${config}`.trim();
    }
    // The in-app string shouldn't compete with a stale config-file path.
    if (this.env["MANGOHUD_CONFIGFILE"]) this.env["MANGOHUD_CONFIGFILE"].enabled = false;
    if (this.wrap["mangohud"]) this.wrap["mangohud"].enabled = true;
    this.mark("apply MangoHud preset");
  }

  applyMangoFile(path: string) {
    if (this.env["MANGOHUD_CONFIGFILE"]) {
      this.env["MANGOHUD_CONFIGFILE"] = { enabled: true, value: path };
    } else {
      // Fall back to custom env if the catalog lacks the key.
      this.extraEnv = `${this.extraEnv} MANGOHUD_CONFIGFILE=${path}`.trim();
    }
    // MANGOHUD_CONFIG takes priority over config files — disable it so the
    // file's settings actually take effect.
    if (this.env["MANGOHUD_CONFIG"]) this.env["MANGOHUD_CONFIG"].enabled = false;
    if (this.wrap["mangohud"]) this.wrap["mangohud"].enabled = true;
    this.mark("use MangoHud config file");
  }

  // ------------------------------ optiscaler --------------------------------

  /**
   * Apply a composed OptiScaler.ini config string, enabling OptiScaler
   * injection so the config has an effect. An empty string still enables
   * injection but clears the config (back to OptiScaler's own defaults).
   *
   * `proxy` is the DLL OptiScaler injects as (`PROTON_OPTISCALER_NAME`); blank
   * means "leave it at OptiScaler's default", which is expressed by turning the
   * row off rather than writing `dxgi.dll` explicitly — the builder shouldn't
   * add a variable that changes nothing.
   */
  applyOptiScaler(config: string, proxy = "") {
    if (this.env["PROTON_OPTISCALER_CONFIG"]) {
      this.env["PROTON_OPTISCALER_CONFIG"] = { enabled: config !== "", value: config };
    } else if (config) {
      // Fall back to custom env if the catalog lacks the key.
      this.extraEnv = `${this.extraEnv} PROTON_OPTISCALER_CONFIG='${config}'`.trim();
    }
    if (this.env["PROTON_OPTISCALER_NAME"]) {
      this.env["PROTON_OPTISCALER_NAME"] = { enabled: proxy !== "", value: proxy };
    } else if (proxy) {
      this.extraEnv = `${this.extraEnv} PROTON_OPTISCALER_NAME=${proxy}`.trim();
    }
    if (this.env["PROTON_USE_OPTISCALER"]) this.env["PROTON_USE_OPTISCALER"].enabled = true;
    this.mark("apply OptiScaler config");
  }

  // ------------------------------- recipes ----------------------------------

  async applyRecipe(index: number) {
    const recipe = this.recipes[index];
    const cfg = await ipc.applyRecipe(index, this.toConfig());
    this.loadConfig(cfg);
    // loadConfig → resetOptions clears the map, so attribute after, not before.
    if (recipe) {
      for (const [key] of [...recipe.env, ...recipe.wrappers]) {
        this.recipeOrigin[key] = recipe.name;
      }
    }
    // The most destructive action in the app: recipes.rs is additive-only, so
    // stacking them accumulates with no way back short of a reset.
    this.mark(`apply "${this.recipes[index]?.name ?? "recipe"}"`);
  }

  // ------------------------------- theme/store ------------------------------

  setTheme(id: string) {
    this.store.theme = id;
    applyTheme(id);
    this.persistStore();
  }

  /** Navigate the Console main panel; clears any active parameter search. */
  setSection(section: string) {
    this.activeSection = section;
    this.paramQuery = "";
  }

  /**
   * Whether the "Apply to Heroic?" confirmation is up.
   *
   * Lives on the store rather than inside `LauncherAction` because that button
   * is mounted at two call sites at once and both unmount on a routine view
   * change. The dialog it drives is mounted once, at the app root
   * (`HeroicConfirm`), so a view change can never destroy an open bits-ui modal
   * and strand `body { pointer-events: none }`.
   */
  heroicConfirmOpen = $state(false);

  /**
   * Whether the "Set as system MangoHud default?" confirmation is up, and the
   * MANGOHUD_CONFIG-style string it would export if confirmed (the MangoHud
   * dialog's own live builder output, stashed here when its button is
   * clicked). Same rationale as `heroicConfirmOpen`: the dialog is mounted
   * once at the app root (`MangoHudSystemConfirm`) rather than beside its
   * trigger, so it survives the trigger's own dialog closing mid-flow.
   */
  mangoSystemConfirmOpen = $state(false);
  pendingMangoSystemConfig = $state("");

  /**
   * The row `revealParam` last asked for. `OptionRow` watches this and scrolls,
   * focuses and flashes itself on a match.
   *
   * The nonce is load-bearing: a bare `string | null` would not change when the
   * same key is requested twice, so clicking the same lint notice a second time
   * would silently do nothing.
   */
  focusParam = $state<{ key: string; nonce: number } | null>(null);
  private focusNonce = 0;

  /**
   * Navigate to, scroll to and focus a parameter by catalog key. Built once here
   * because lint click-to-jump (#48) and the command palette (#54) both need it.
   *
   * Returns false when the key isn't in the catalog at all, so a caller can say
   * so rather than appearing to do nothing.
   */
  revealParam(key: string): boolean {
    const env = this.catalog.envs.find((e) => e.key === key);
    const wrapper = env ? null : this.catalog.wrappers.find((w) => w.key === key);
    const def = env ?? wrapper;
    if (!def) return false;

    // The relevance guard, and the non-obvious part of this whole primitive:
    // MainPanel filters hardware-irrelevant rows out entirely, so jumping to one
    // would land on nothing. Not hypothetical — the nvapi-without-nvidia notice
    // is *precisely* about a hardware-irrelevant option, so its jump link would
    // fail exactly when it matters most.
    if (!this.store.show_irrelevant && irrelevance(this.hwCaps, def.gpu, def.needs)) {
      this.setShowIrrelevant(true);
    }
    // Same problem, second filter: the advanced tier hides rows too, and a lint
    // fix or palette jump has no reason to respect a tidiness preference.
    if (!this.store.show_advanced && isAdvanced(def)) {
      this.setShowAdvanced(true);
    }

    this.setSection(env ? env.category : "Wrappers");
    this.focusParam = { key, nonce: ++this.focusNonce };
    return true;
  }

  /**
   * Hardware facts plus the opt-in HDR/FSR/GPU-generation capabilities, for
   * relevance filtering. `fsr4` is true for either RDNA generation (with the
   * legacy `store.fsr4` flag as a fallback for pre-`gpu_gen` state files);
   * `rdna3` and `rdna4` are exclusive, so each generation's options hide on the
   * other.
   *
   * Both generation flags are gated on `hardware.amd`. `gpu_gen` is a persisted
   * free string that nothing re-validates against the detected GPU, so a state
   * file carried to an NVIDIA machine would otherwise keep unlocking AMD-only
   * rows — including `PROTON_FSR4_INDICATOR`, which had no `gpu` hint of its own.
   */
  get hwCaps(): HwCaps {
    const gen = this.store.gpu_gen;
    const amd = this.hardware.amd;
    return {
      ...this.hardware,
      hdr: this.store.hdr,
      fsr4: amd && (gen === "rdna3" || gen === "rdna4" || this.store.fsr4),
      rdna3: amd && gen === "rdna3",
      rdna4: amd && gen === "rdna4",
    };
  }

  /** Catalog env keys tagged as a good default for the current GPU
   *  capabilities that aren't already on at their recommended value — what
   *  `applyRecommendedForGpu` would still change. Empty means the button has
   *  nothing to do (either untagged hardware, or already applied). */
  get recommendedEnvKeys(): string[] {
    const caps = this.hwCaps;
    return this.catalog.envs
      .filter((d) => isRecommended(caps, d.recommended_for))
      .filter((d) => {
        const s = this.env[d.key];
        if (!s) return false;
        return !s.enabled || (d.default_value !== "" && s.value !== d.default_value);
      })
      .map((d) => d.key);
  }

  /** One-click "Recommended for your GPU": batch-enable every catalog param
   *  tagged `recommended_for` a currently-true capability, at its documented
   *  default. Never runs on its own — the frontend never mutates config
   *  without a click, same as a recipe. */
  applyRecommendedForGpu() {
    const keys = new Set(this.recommendedEnvKeys);
    if (!keys.size) return;
    for (const d of this.catalog.envs) {
      if (!keys.has(d.key)) continue;
      const s = this.env[d.key];
      if (!s) continue;
      s.enabled = true;
      if (d.default_value !== "") s.value = d.default_value;
      this.disownParam(d.key);
    }
    this.mark("apply GPU-recommended defaults");
  }

  /** The UI density mode, normalised: anything other than "advanced" (including
   *  the "" an older state.toml carries) is "simple". */
  get uiMode(): UiMode {
    return this.store.ui_mode === "advanced" ? "advanced" : "simple";
  }
  setUiMode(m: UiMode) {
    this.store.ui_mode = m;
    this.persistStore();
  }

  /** Dismiss the Simple-mode first-run tour, permanently (finished or skipped —
   *  both count as "seen", there's no "show me again"). IntroTour.svelte owns
   *  whether the dialog is actually open; this only persists the flag. */
  markTourSeen() {
    if (this.store.seen_intro_tour) return;
    this.store.seen_intro_tour = true;
    this.persistStore();
  }

  setShowIrrelevant(v: boolean) {
    this.store.show_irrelevant = v;
    this.persistStore();
  }
  setShowAdvanced(v: boolean) {
    this.store.show_advanced = v;
    this.persistStore();
  }
  setHdr(v: boolean) {
    this.store.hdr = v;
    this.persistStore();
  }
  setFsr4(v: boolean) {
    this.store.fsr4 = v;
    this.persistStore();
  }
  /** Set the AMD GPU generation. Clears the legacy `fsr4` flag once a
   *  generation is chosen so the two can't disagree. */
  setGpuGen(gen: GpuGen) {
    this.store.gpu_gen = gen;
    if (gen) this.store.fsr4 = false;
    this.persistStore();
  }
  setProtondbAuto(v: boolean) {
    this.store.protondb_auto = v;
    this.persistStore();
  }
  setLlmEnabled(v: boolean) {
    this.store.llm_enabled = v;
    this.persistStore();
  }
  setLlmEndpoint(v: string) {
    this.store.llm_endpoint = v;
    this.persistStore();
  }
  setLlmModel(v: string) {
    this.store.llm_model = v;
    this.persistStore();
  }

  // ------------------------------ paths -------------------------------------

  /** Replace one of the path lists and re-scan. */
  setPathList(field: "steam_roots" | "steam_libraries" | "proton_dirs", list: string[]) {
    this.store.paths[field] = list;
    this.persistStore();
    this.scheduleRescan();
  }

  /** Override the program token emitted for `name` ("" clears the override). */
  setBinOverride(name: string, value: string) {
    if (value.trim() === "") delete this.store.paths.bins[name];
    else this.store.paths.bins[name] = value;
    this.persistStore();
    this.scheduleRescan();
  }

  private rescanTimer: ReturnType<typeof setTimeout> | undefined;

  /**
   * Debounced discovery re-scan. The rescan *is* the validator for a configured
   * path — there is no separate validate command, which would be a second
   * implementation of the same scan that could disagree with it. Debounced
   * because these setters fire per keystroke and a scan hits the filesystem.
   */
  private scheduleRescan() {
    clearTimeout(this.rescanTimer);
    this.rescanTimer = setTimeout(() => void this.refresh(), 600);
  }

  /** Path warnings only — the parse warnings belong to the TOML overrides. */
  get pathWarnings() {
    return this.configWarnings.filter((w) => w.kind === "path");
  }

  // ------------------------------ library view ------------------------------

  isFavorite(appId: number): boolean {
    return this.store.favorites.includes(appId);
  }

  toggleFavorite(appId: number) {
    // Mirrors a Rust BTreeSet: no duplicates, and kept sorted so the persisted
    // TOML stays stable rather than reordering on every toggle.
    const next = this.store.favorites.filter((id) => id !== appId);
    if (next.length === this.store.favorites.length) next.push(appId);
    next.sort((a, b) => a - b);
    this.store.favorites = next;
    this.persistStore();
  }

  /** The persisted sort, falling back when the stored string is empty (first
   *  run) or an id written by a newer build. */
  get librarySort(): LibrarySort {
    const s = this.store.library_sort;
    return s === "recent" || s === "alpha" || s === "tuned" ? s : DEFAULT_LIBRARY_SORT;
  }

  setLibrarySort(s: LibrarySort) {
    this.store.library_sort = s;
    this.persistStore();
  }

  // -------------------------------- game art --------------------------------

  private artKey(appId: number, source: string, kind: ArtKind): string {
    return `${source}:${appId}:${kind}`;
  }

  /** Cached art (data URL), `null` if none found, `undefined` if not loaded. */
  artFor(appId: number, source: string, kind: ArtKind): string | null | undefined {
    return this.artCache[this.artKey(appId, source, kind)];
  }

  /**
   * Lazily fetch a game's art once; result lands in `artCache` reactively.
   *
   * `artHint` is `GameDto.art_url` — a Heroic sideload's own `art_cover` /
   * `art_square` (a `file://` path or remote URL). It's the only lead the
   * backend has on a Heroic game's art, since a sideload has no Steam appid a
   * cache lookup could key off; every other source ignores it.
   */
  requestArt(appId: number, source: string, kind: ArtKind, artHint?: string | null) {
    const key = this.artKey(appId, source, kind);
    if (this.artRequested.has(key)) return;
    this.artRequested.add(key);

    // "Local + online fallback" per the user's choice: allow the CDN backstop.
    const run = () => {
      this.artInFlight++;
      ipc
        .gameArt(appId, source, kind, true, artHint ?? null)
        .then((url) => (this.artCache[key] = url))
        .catch(() => (this.artCache[key] = null))
        .finally(() => {
          this.artInFlight--;
          this.artQueue.shift()?.();
        });
    };

    if (this.artInFlight < ART_CONCURRENCY) run();
    else this.artQueue.push(run);
  }

  /**
   * Retry art that previously came back empty.
   *
   * `artRequested` is a permanent "don't ask twice" set, which is right for the
   * steady state but means a lookup that failed once stays blank for the rest of
   * the session. A library refresh is the natural moment to try again — but only
   * for the failures: successes stay cached so a refresh doesn't re-fetch every
   * tile the user can already see.
   */
  private retryFailedArt() {
    for (const [key, value] of Object.entries(this.artCache)) {
      if (value === null) {
        delete this.artCache[key];
        this.artRequested.delete(key);
      }
    }
    // Anything still queued was scheduled against the pre-refresh library.
    this.artQueue.length = 0;
  }

  // ------------------------------- protondb ---------------------------------

  /**
   * Session cache for ProtonDB tiers, mirroring the art cache above: switching
   * between two games used to refetch both every time, which is needless load on
   * an unofficial third-party API of unknown rate limits.
   *
   * Session-only on purpose. A persisted TTL cache needs a `Store` field, a
   * clock, an eviction policy and `Tier: Deserialize` — worth measuring for
   * first, and it would have to respect the wholesale-overwrite hazard in #43.
   */
  tierCache = $state<Record<string, Tier | null>>({});
  private tierRequested = new Set<number>();
  /** Appids with a fetch in flight, so the chip can show a spinner. */
  tierLoading = $state<Record<string, boolean>>({});

  /** Cached tier, `null` if the lookup failed, `undefined` if never requested. */
  tierFor(appId: number): Tier | null | undefined {
    return this.tierCache[String(appId)];
  }

  /** Fetch a tier at most once per session. Safe to call repeatedly. */
  requestTier(appId: number) {
    if (this.tierRequested.has(appId)) return;
    this.tierRequested.add(appId);
    this.tierLoading[String(appId)] = true;
    ipc
      .protondbFetch(appId)
      .then((t) => (this.tierCache[String(appId)] = t))
      .catch((e) => {
        console.error("protondbFetch failed", appId, e);
        this.tierCache[String(appId)] = null;
      })
      .finally(() => (this.tierLoading[String(appId)] = false));
  }

  /** Drop a failed lookup so the chip's Retry can try again. */
  retryTier(appId: number) {
    this.tierRequested.delete(appId);
    delete this.tierCache[String(appId)];
    this.requestTier(appId);
  }

  // ------------------------- OptiScaler upgrade -------------------------

  /** Session cache: appid -> whether an existing OptiScaler install was found
   *  in that game's folder, mirroring the ProtonDB tier cache above so
   *  switching games doesn't re-stat the filesystem every time. */
  optiscalerStatusCache = $state<Record<string, OptiscalerStatus>>({});
  private optiscalerStatusRequested = new Set<number>();
  optiscalerStatusLoading = $state<Record<string, boolean>>({});

  optiscalerStatusFor(appId: number): OptiscalerStatus | undefined {
    return this.optiscalerStatusCache[String(appId)];
  }

  /** Check at most once per session per game. Safe to call repeatedly. */
  requestOptiscalerStatus(appId: number) {
    if (this.optiscalerStatusRequested.has(appId)) return;
    this.optiscalerStatusRequested.add(appId);
    this.optiscalerStatusLoading[String(appId)] = true;
    ipc
      .optiscalerStatus(appId)
      .then((s) => (this.optiscalerStatusCache[String(appId)] = s))
      .catch((e) => {
        console.error("optiscalerStatus failed", appId, e);
        this.optiscalerStatusCache[String(appId)] = { install_dir: null, found: false };
      })
      .finally(() => (this.optiscalerStatusLoading[String(appId)] = false));
  }

  /** The latest upstream OptiScaler release — global, not per-game, so it's
   *  fetched at most once per session regardless of which game is open. */
  optiscalerLatest = $state<OptiscalerRelease | null>(null);
  optiscalerLatestLoading = $state(false);
  optiscalerLatestError = $state<string | null>(null);
  private optiscalerLatestRequested = false;

  requestOptiscalerLatest() {
    if (this.optiscalerLatestRequested) return;
    this.optiscalerLatestRequested = true;
    this.optiscalerLatestLoading = true;
    this.optiscalerLatestError = null;
    ipc
      .optiscalerLatest()
      .then((r) => (this.optiscalerLatest = r))
      .catch((e) => (this.optiscalerLatestError = String(e)))
      .finally(() => (this.optiscalerLatestLoading = false));
  }

  optiscalerFetchBusy = $state(false);

  /** Download the latest OptiScaler release and extract it into `appId`'s
   *  install directory. The one action in the app that writes into a game's
   *  own folder — callers gate this behind an explicit confirm, never call it
   *  from a $effect or on load. Throws on failure; caller toasts. */
  async fetchOptiscalerUpgrade(appId: number): Promise<OptiscalerExtractResult> {
    this.optiscalerFetchBusy = true;
    try {
      const result = await ipc.optiscalerFetch(appId);
      const prev = this.optiscalerStatusCache[String(appId)];
      this.optiscalerStatusCache[String(appId)] = { install_dir: prev?.install_dir ?? null, found: true };
      return result;
    } finally {
      this.optiscalerFetchBusy = false;
    }
  }

  /**
   * Send the current game's log (plus the built command) to the local LLM and
   * store its suggestions. The backend adds the catalog allow-list and hardware
   * summary and reads the endpoint/model from the store; we just forward the log
   * content already on screen. Advisory only — applying a suggested change is a
   * separate, explicit user click (`applyLlmChange`).
   */
  async analyzeLog(log: {
    error_lines: string[];
    tail: string;
  }): Promise<void> {
    this.aiLoading = true;
    this.aiError = null;
    try {
      this.aiResult = await ipc.llmAnalyze({
        command: this.command,
        game_name: this.selectedGameName ?? "",
        error_lines: log.error_lines,
        log_tail: log.tail,
      });
    } catch (e) {
      console.error("llmAnalyze failed", e);
      this.aiError = String(e);
      this.aiResult = null;
    } finally {
      this.aiLoading = false;
    }
  }

  /** Clear the last analysis (e.g. when the log dialog closes or the game
   *  changes) so a stale suggestion never shows against a different game. */
  clearAnalysis() {
    this.aiResult = null;
    this.aiError = null;
    this.aiLoading = false;
  }

  /**
   * Diagnose a free-text symptom. Pulls the current game's log in as optional
   * context (the diagnosis is better with it, but works without — the user may
   * be troubleshooting before a first launch), then asks the backend, which
   * offers the Fix recipes and catalog allow-list to constrain the answer.
   */
  async troubleshoot(symptom: string): Promise<void> {
    this.tsLoading = true;
    this.tsError = null;
    try {
      let errorLines: string[] = [];
      let hasLog = false;
      if (this.selectedAppId != null) {
        try {
          const log = await ipc.readProtonLog(this.selectedAppId);
          if (log.present) {
            hasLog = true;
            errorLines = log.error_lines;
          }
        } catch {
          // The log is optional context; a read failure must not block the
          // symptom-only diagnosis.
        }
      }
      this.tsResult = await ipc.llmTroubleshoot({
        symptom,
        command: this.command,
        game_name: this.selectedGameName ?? "",
        error_lines: errorLines,
        has_log: hasLog,
      });
    } catch (e) {
      console.error("llmTroubleshoot failed", e);
      this.tsError = String(e);
      this.tsResult = null;
    } finally {
      this.tsLoading = false;
    }
  }

  /** Reset the troubleshooter (e.g. when its dialog closes). */
  clearTroubleshoot() {
    this.tsResult = null;
    this.tsError = null;
    this.tsLoading = false;
  }

  /**
   * Apply one AI-suggested change by toggling the catalog key it names. Only
   * keys the catalog actually has can be applied; `kind` is re-derived from the
   * live env/wrapper maps rather than trusting the model's hint. Returns whether
   * the change was applied.
   */
  applyLlmChange(change: { key: string; value: string }): boolean {
    if (this.env[change.key]) {
      if (!this.env[change.key].enabled) this.toggleEnv(change.key);
      if (change.value) this.setEnvValue(change.key, change.value);
      return true;
    }
    if (this.wrap[change.key]) {
      if (!this.wrap[change.key].enabled) this.toggleWrap(change.key);
      if (change.value) this.setWrapValue(change.key, change.value);
      return true;
    }
    return false;
  }

  /** True when the catalog has the key an AI change names, so the UI can show a
   *  clickable "Apply" chip (vs. plain advisory text for unknown keys). */
  hasCatalogKey(key: string): boolean {
    return !!this.env[key] || !!this.wrap[key];
  }

  dismissStale() {
    if (this.stale) {
      this.store.dismissed_cachyos_build = this.stale.installed;
      this.persistStore();
    }
  }

  get staleVisible(): boolean {
    return (
      this.stale != null &&
      this.store.dismissed_cachyos_build !== this.stale.installed
    );
  }

  // ------------------------------- updates ----------------------------------

  /**
   * Returns what happened, so a user-initiated check can report "you're
   * already up to date" — the silent startup check has nothing to say in
   * that case, but someone who pressed a button is owed an answer.
   */
  async checkForUpdate(): Promise<"available" | "up-to-date" | "failed"> {
    try {
      const info = await ipc.checkForUpdate();
      if (info?.available) {
        this.update = info;
        // A previous dismissal shouldn't suppress a check the user asked for.
        if (this.store.dismissed_update_version === info.latest) {
          this.store.dismissed_update_version = "";
        }
        return "available";
      }
      return "up-to-date";
    } catch (e) {
      console.error("update check failed", e);
      return "failed";
    }
  }

  get updateVisible(): boolean {
    return (
      this.update != null &&
      this.store.dismissed_update_version !== this.update.latest
    );
  }

  dismissUpdate() {
    if (this.update) {
      this.store.dismissed_update_version = this.update.latest;
      this.persistStore();
    }
  }

  /** Download, verify and swap the new binary. On success the backend restarts
   *  the app into the new version, so this never returns in the real shell. */
  async applyUpdate() {
    if (!this.update) return;
    this.updating = true;
    try {
      await ipc.runUpdate($state.snapshot(this.update));
    } finally {
      this.updating = false;
    }
  }

  persistStore() {
    // Fire and forget; the store is small.
    ipc
      .saveStore($state.snapshot(this.store))
      .then(() => {
        // Recovered — drop the banner so it can't linger once writes work.
        this.persistError = null;
      })
      .catch((e) => {
        console.error("saveStore failed", e);
        const message = String(e);
        this.persistError = message;

        // persistStore fires on a debounce during typing, so a broken config
        // dir would otherwise produce a toast storm. The sticky banner is the
        // durable signal; the toast just draws the eye, once per minute.
        const now = Date.now();
        if (now - this.lastPersistToast > 60_000) {
          this.lastPersistToast = now;
          toast.error("Couldn't save your settings");
        }
      });
  }
}

export const app = new AppStore();

export function freshConfig(): Config {
  return emptyConfig();
}
