import { untrack } from "svelte";
import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { ipc } from "./ipc";
import { toast } from "./toast.svelte";
import { history } from "./history.svelte";
import { art } from "./art.svelte";
import { lookups } from "./lookups.svelte";
import type { Entry, Snapshot } from "./history.svelte";
import { applyTheme, DEFAULT_THEME } from "./themes";
import { formatExtraEnv, mergeIntoExtraEnv, setInExtraEnv, splitExtraEnv } from "./shell";
import { irrelevance, isRecommended } from "./util";
import { isAdvanced, withoutLaunchTarget } from "./types";
import { LSFG_BUILDER_KEYS } from "./lsfg";
import { encodePreset } from "./presetCode";
import { buildOptiScaler, parseOptiScaler } from "./optiscaler";
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
  LsfgStatus,
  OptiscalerChannel,
  OptiscalerExtractResult,
  OptiscalerRelease,
  OptiscalerStatus,
  Preset,
  Recipe,
  RuntimeDto,
  ConfigWarning,
  StaleInfo,
  Store,
  SyncState,
  Token,
  TroubleshootResult,
  UiMode,
  RuntimeUpdate,
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
  dismissed_runtime_updates: [],
  show_irrelevant: false,
  show_advanced: false,
  hdr: false,
  fsr4: false,
  gpu_gen: "",
  protondb_auto: false,
  anticheat_check: false,
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

/** Single source of truth for the whole UI, backed by Svelte 5 runes. */
class AppStore {
  // ---- bootstrap data (mostly immutable) ----
  ready = $state(false);
  loadError = $state<string | null>(null);
  /** Set only when Steam was found but zero Proton runtimes exist anywhere. */
  runtimeWarning = $state<string | null>(null);
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
    gpu_gen_detected: null,
    monitors: [],
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
  /** Proton families with a newer upstream build than the newest installed. */
  runtimeUpdates = $state<RuntimeUpdate[]>([]);
  updating = $state(false);
  /** True while a library re-scan (rescan IPC) is in flight. */
  refreshing = $state(false);

  // ---- focus refresh of Steam's launch options (see refreshSteamConfig) ----
  /** Until when (epoch ms) the user counts as "about to paste into Steam":
   *  set by Copy / Open in Steam, so focus also schedules follow-up re-reads. */
  private awaitingPasteUntil = 0;
  private steamReadInFlight = false;
  private lastSteamRead = 0;
  private followUps: ReturnType<typeof setTimeout>[] = [];
  /** A re-read during the paste window changed the options; announce the next
   *  diff if it lands in sync. */
  private announceApplied = false;
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
  /** Which Settings sections are expanded. On the store (per session) so a
   *  deep link can open the one it's pointing at. */
  settingsSections = $state<Record<SettingsSection, boolean>>({
    appearance: false,
    behavior: false,
    ai: false,
    paths: false,
    about: false,
  });
  /** Scroll request for the drawer; the nonce re-triggers a repeat jump. */
  settingsFocus = $state<{ section: SettingsSection; nonce: number } | null>(null);
  /** Bumped to replay the intro tour (IntroTour watches it). */
  tourReplay = $state(0);
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
    this.runtimeWarning = b.runtime_warning;
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

    // Needs `games` (each entry's own exe) and the default runtime, both set above.
    this.adoptAliases();
    this.repairGameMemory();

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

    // Opened as `protongen --game <id>` (Nexus's "Tune in protongen"): start on
    // that game instead of the restored one.
    if (b.initial_game_appid != null) this.openRequestedGame(b.initial_game_appid);

    // Seed the undo baseline with the state the user is actually looking at, so
    // restoring a session isn't itself the first undo entry.
    history.reset(this.snapshot());

    this.ready = true;

    // Badge the library grid. After `ready` so the first paint isn't waiting on
    // it — the grid renders unbadged and fills in.
    this.refreshLaunchStatuses();

    // Check for a newer release in the background; never blocks launch.
    this.checkForUpdate();
    this.checkRuntimeUpdates();
    // Lossless Scaling profile names for the LSFGVK_PROFILE row; cheap, local.
    void this.refreshLsfgStatus();
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
      // Steam's side changing (a rescan, or the focus refresh) only needs the
      // sync diff re-run. Reading it in the effect above also re-ran the session
      // persist, which wrote game_memory and flashed "Saved" on every refocus.
      $effect(() => {
        void this.currentLaunchOptions;
        untrack(() => this.scheduleRecompute(this.toConfig()));
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
    if (this.refreshing) {
      // Still scan once more afterwards: this call may carry a newer input
      // (a Settings path typed after the running scan started) that the
      // running scan never saw.
      this.rescanPending = true;
      return "busy";
    }
    this.refreshing = true;
    this.rescanPending = false;
    try {
      const b = await ipc.rescan();
      this.loadError = b.load_error;
      this.runtimeWarning = b.runtime_warning;
      this.steamRoot = b.steam_root;
      this.runtimes = this.withAutoRuntime(b.runtimes);
      this.games = b.games;
      this.launchOptions = b.launch_options;
      this.compatTools = b.compat_tools;
      this.stale = b.stale;
      // Cheap after the first run: the backend caches upstream releases, so
      // this only re-compares against the freshly scanned runtimes.
      this.checkRuntimeUpdates();
      // A rescan is how a freshly installed Lossless Scaling's DLL is found.
      void this.refreshLsfgStatus();
      // Both are recomputed by `rescan` and must be copied, or a corrected
      // Settings path would never clear its banner and a fixed binary override
      // would never turn its badge green.
      this.configWarnings = b.config_warnings;
      this.requiresStatus = b.requires_status;
      // A repack Nexus added since the last scan now stands in for its mirrors.
      this.adoptAliases();

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
      if (this.rescanPending) void this.refresh();
    }
    // A refresh is the user asking for a fresh look, so give art that came back
    // empty another chance rather than leaving those tiles blank all session.
    art.retryFailed();
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
    // Fired concurrently from refresh, backToLibrary and the focus re-read;
    // only the newest call's answer may land, or older badges overwrite newer.
    const seq = ++this.statusSeq;
    try {
      const statuses = await ipc.launchStatuses(
        $state.snapshot(this.store.game_memory),
        $state.snapshot(this.launchOptions),
      );
      if (seq === this.statusSeq) this.launchStatuses = statuses;
    } catch (e) {
      console.error("launchStatuses failed", e);
    }
  }
  private statusSeq = 0;
  private applyingRecipe = false;
  /** A `refresh` arrived while one was running; run once more when it ends. */
  private rescanPending = false;

  /** The runtime a Nexus game is pinned to (Nexus names it by folder), or
   *  null when it isn't one protongen found. */
  private runtimeNamed(name: string | null): RuntimeDto | null {
    if (!name) return null;
    return (
      this.runtimes.find(
        (r) => r.path.replace(/\/+$/, "").split("/").pop() === name || r.internal_name === name,
      ) ?? null
    );
  }

  /** What opening `game` with nothing saved looks like. Heroic and Nexus
   *  launch through umu/Proton themselves, so Steam mode's `%command%` means
   *  nothing for them; a Nexus game also starts on its own prefix and the
   *  Proton build Nexus runs it with — without the prefix, a copied umu
   *  command ran the repack in umu's default prefix. */
  private freshLaunch(game: GameDto | null) {
    const nexus = game?.source === "nexus";
    return {
      umu: game?.source === "heroic" || nexus,
      exe: game?.executable ?? "",
      prefix: (nexus && game?.wine_prefix) || "",
      runtime:
        (nexus && this.runtimeNamed(game?.pinned_proton ?? null)) ||
        this.steamMappedRuntime(game) ||
        this.defaultRuntime(),
    };
  }

  /** The runtime Steam's Proton dropdown already has for `game`, when it is
   *  one protongen found. Starting a fresh game there means the command and
   *  Steam agree from the first frame, instead of every untouched game opening
   *  on a "change Steam's Proton to proton-cachyos" correction. Valve and auto
   *  runtimes carry placeholder internal names, so they can never match. */
  private steamMappedRuntime(game: GameDto | null): RuntimeDto | null {
    if (game?.source !== "steam") return null;
    const tool = this.compatTools[String(game.app_id)];
    if (!tool) return null;
    return (
      this.runtimes.find(
        (r) => r.kind !== "valve" && r.kind !== "auto" && r.internal_name === tool,
      ) ?? null
    );
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

  /**
   * Set one env row's state without recording history.
   *
   * Every public mutator is this plus a `mark()`; `applyBundle` is N of these
   * plus a single `mark()`. A blank `value` means "leave the value alone", which
   * is what turning a row off must do — the value is still the user's.
   */
  private applyEnv(key: string, enabled: boolean, value = "") {
    const s = this.env[key];
    if (!s) return;
    s.enabled = enabled;
    if (value) s.value = value;
    this.disownParam(key);
  }

  private applyWrap(key: string, enabled: boolean, value = "") {
    const s = this.wrap[key];
    if (!s) return;
    s.enabled = enabled;
    if (value) s.value = value;
    this.disownParam(key);
  }

  toggleEnv(key: string) {
    const s = this.env[key];
    if (!s) return;
    const next = !s.enabled;
    this.applyEnv(key, next);
    this.mark(`${next ? "enable" : "disable"} ${key}`);
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
    const next = !s.enabled;
    this.applyWrap(key, next);
    this.mark(`${next ? "enable" : "disable"} ${key}`);
  }
  setWrapValue(key: string, value: string) {
    const s = this.wrap[key];
    if (!s) return;
    s.value = value;
    this.disownParam(key);
    history.note(`set ${key}`);
  }

  /** Apply the gamescope builder's arguments: turn the wrapper on with them,
   *  as one undo step. Empty args fall back to `-f`, the catalog default — a
   *  bare `gamescope --` would open a 1280x720 window. */
  applyGamescope(args: string) {
    const s = this.wrap["gamescope"];
    if (!s) return;
    s.enabled = true;
    s.value = args.trim() || "-f";
    this.disownParam("gamescope");
    this.mark("configure gamescope");
  }

  /** Whether the free-text game-arguments field carries `token` as a whole
   *  argument. Token-wise, not a substring test: `--dx11` must not report true
   *  because `--dx11-only` is present. */
  hasGameArg(token: string): boolean {
    return this.gameArgs.split(/\s+/).includes(token);
  }

  /** Add or remove one exact argument token, preserving everything else. */
  private applyGameArg(token: string, on: boolean) {
    const tokens = this.gameArgs.split(/\s+/).filter(Boolean);
    if (tokens.includes(token) === on) return;
    this.gameArgs = (on ? [...tokens, token] : tokens.filter((t) => t !== token)).join(" ");
  }

  /**
   * Turn a whole curated bundle on or off as **one** undoable action.
   *
   * Simple mode's cards each own more than one thing: the FSR 4 card sets two
   * env vars, the ray-tracing card an env var *and* a game-argument token.
   * Driving them through the public per-key mutators pushed a separate history
   * entry per key, so one undo left the card half-applied — and the game-arg
   * write, being a bare assignment, was never marked at all and got swallowed by
   * whatever coalescing burst happened to be open, labelled "edit".
   */
  applyBundle(
    label: string,
    on: boolean,
    bundle: { env?: [string, string][]; wrappers?: [string, string][]; gameArg?: string },
  ) {
    // Values are only written on the way *on*: turning a bundle off must not
    // rewrite a value the user has since edited.
    for (const [key, value] of bundle.env ?? []) this.applyEnv(key, on, on ? value : "");
    for (const [key, value] of bundle.wrappers ?? []) this.applyWrap(key, on, on ? value : "");
    if (bundle.gameArg) this.applyGameArg(bundle.gameArg, on);
    this.mark(label);
  }

  /**
   * Turn keys off and on as **one** undoable action — the shape of a lint fix
   * or an AI suggestion. Each key may name an env var or a wrapper. These used
   * to go through the public per-key mutators, one history entry each, so the
   * single Undo on their toast only reverted the last step.
   */
  applyChanges(label: string, changes: { enable?: [string, string][]; disable?: string[] }) {
    for (const key of changes.disable ?? []) {
      this.applyEnv(key, false);
      this.applyWrap(key, false);
    }
    for (const [key, value] of changes.enable ?? []) {
      this.applyEnv(key, true, value);
      this.applyWrap(key, true, value);
    }
    this.mark(label);
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

  /**
   * Everything the user has turned on, for the nav badge and the Active view.
   *
   * `$derived`, not a getter: a class getter is re-evaluated on every read, and
   * `ActiveOptions` reads this three times per render while `NavRail` reads it
   * once — each time walking the whole catalog twice and re-tokenizing the
   * custom-env string. Same reasoning for every `$derived` below.
   */
  activeCount = $derived.by((): number => {
    const envs = this.catalog.envs.filter((e) => this.env[e.key]?.enabled).length;
    const wraps = this.catalog.wrappers.filter((w) => this.wrap[w.key]?.enabled).length;
    return envs + wraps + splitExtraEnv(this.extraEnv).length;
  });

  /** Drop one `K=V` token from the custom-env string. The rest is re-rendered
   *  with its quoting — joining the unquoted tokens turned `FOO="a b"` into
   *  `FOO=a b`, which the shell splits in two. */
  removeExtraEnv(raw: string) {
    const kept = splitExtraEnv(this.extraEnv).filter((p) => p.raw !== raw);
    this.extraEnv = formatExtraEnv(kept.map((p) => [p.key, p.value]));
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
  visibleCategories = $derived.by((): string[] => {
    if (this.store.show_irrelevant) return this.categories;
    const caps = this.hwCaps;
    return this.categories.filter((c) =>
      this.catalog.envs.some((e) => e.category === c && !irrelevance(caps, e.gpu, e.needs)),
    );
  });

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

  /** Replace the selection with `cfg`. `keepLaunchTarget` keeps the current
   *  game's umu mode / exe / prefix / game id — for presets and the global
   *  profile, which are tuning to lay onto whatever game is open. */
  loadConfig(cfg: Config, { keepLaunchTarget = false } = {}) {
    const target = keepLaunchTarget
      ? { umu: this.umu, umu_exe: this.umuExe, umu_wineprefix: this.umuWineprefix, umu_gameid: this.umuGameid }
      : cfg;
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
    this.umu = target.umu;
    // Idempotent with the backend merge: after a round-trip the key is already
    // in `cfg.extra_env`, so it produces no leftover and nothing is duplicated.
    this.extraEnv = mergeIntoExtraEnv(cfg.extra_env, leftover);
    this.gameArgs = cfg.game_args;
    this.umuExe = target.umu_exe;
    this.umuWineprefix = target.umu_wineprefix;
    this.umuGameid = target.umu_gameid;
    // A missing or no-longer-installed runtime falls back to the default, not
    // to whatever the previous game had selected — that leaked into this
    // game's memory and made undo see a phantom edit. Only an overlay (preset,
    // profile) with no runtime of its own keeps the current one.
    const r = cfg.runtime ? this.runtimes.find((x) => x.internal_name === cfg.runtime) : undefined;
    if (r) this.selectedRuntime = r;
    else if (!(keepLaunchTarget && cfg.runtime === null)) this.selectedRuntime = this.defaultRuntime();
  }

  /** Reset the command back to defaults, keeping the selected game. Just another
   *  undoable action now — the old "returns the prior config so the caller can
   *  offer a 2-second undo" contract is gone, replaced by the real stack. */
  resetCommand() {
    this.resetLaunchFields(this.selectedGame);
    this.activePresetName = null;
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
    // A default-profile prompt belongs to the game it was raised for; undoing
    // away from that game must not leave "Apply default" aimed at this one.
    this.pendingDefaultPrompt = false;
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
        const notices = await ipc.lint(cfg, this.selectedAppId);
        if (seq === this.recomputeSeq) this.notices = notices;
      } catch (e) {
        console.error("lint failed", e);
        // Drop the previous config's notices: their one-click fixes were
        // computed against a selection that no longer exists.
        if (seq === this.recomputeSeq) this.notices = [];
      }

      // Third await in the existing debounce rather than a timer of its own.
      // Skipped entirely when there is nothing to compare against, which is the
      // common case (generic builds, shortcuts, umu).
      const current = this.currentLaunchOptions;
      if (built !== null && current !== null) {
        try {
          const diff = await ipc.launchDiff(built, current);
          if (seq === this.recomputeSeq) {
            this.launchDiff = diff;
            if (this.announceApplied) {
              this.announceApplied = false;
              if (diff.status === "in-sync") {
                this.awaitingPasteUntil = 0;
                toast.success("Applied in Steam");
              }
            }
          }
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
      this.rememberCurrent();
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

  /** A game asked for from outside — `protongen --game <id>` at startup, or the
   *  same command run again while this window is open (single-instance
   *  handoff). Opens it in the builder; an id protongen doesn't know is said
   *  out loud rather than silently leaving the last game up, since the caller
   *  (Nexus's "Tune in protongen") has no other way to learn it missed. */
  openRequestedGame(appid: number) {
    const g = this.games.find((x) => x.app_id === appid);
    if (g) {
      this.openGame(g);
      return;
    }
    // Rescan re-asks for this same id once the fresh scan lands, so a game
    // added after startup opens without a second trip through Nexus.
    toast.error(`Game ${appid} isn't in protongen's library`, {
      ms: 8000,
      action: {
        label: "Rescan",
        onClick: () =>
          void this.refresh().then((r) => r === "ok" && this.openKnownGame(appid)),
      },
    });
  }

  /** `openRequestedGame` without the not-found toast — the retry after a rescan
   *  shouldn't nag twice. */
  private openKnownGame(appid: number) {
    const g = this.games.find((x) => x.app_id === appid);
    if (g) this.openGame(g);
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
    this.rememberCurrent();
    this.refreshLaunchStatuses();
  }

  /** The selected game, resolved against the discovery list. */
  selectedGame = $derived.by((): GameDto | null => {
    if (this.selectedAppId == null) return null;
    return this.games.find((g) => g.app_id === this.selectedAppId) ?? null;
  });

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

  /** The user just copied the command or opened Steam's properties: for the
   *  next 10 minutes, coming back to the window also re-checks a little later. */
  expectPaste() {
    this.awaitingPasteUntil = Date.now() + 10 * 60_000;
  }

  /**
   * Re-read Steam's launch options and compat tools so the sync pill and the
   * library badges catch up after the user pastes into Steam — no manual
   * refresh. Throttled (`force` skips it, for "Re-check" and the follow-ups).
   *
   * Stale data is harmless by construction: every read comes straight from
   * disk, so the verdict is never older than the last read and can never claim
   * "Applied" early. If Steam flushes localconfig.vdf late, the pill just keeps
   * its previous verdict until a later read.
   */
  async refreshSteamConfig(force = false) {
    if (!this.ready || this.refreshing || this.steamReadInFlight || this.steamRoot == null) return;
    const now = Date.now();
    if (!force && now - this.lastSteamRead < 2000) return;
    this.lastSteamRead = now;
    this.steamReadInFlight = true;
    const wasInSync = this.syncState === "in-sync";
    try {
      const fresh = await ipc.steamUserConfig();
      if (!fresh) return;
      const optionsChanged = !sameEntries(this.launchOptions, fresh.launch_options);
      if (optionsChanged) this.launchOptions = fresh.launch_options;
      if (!sameEntries(this.compatTools, fresh.compat_tools)) this.compatTools = fresh.compat_tools;
      if (optionsChanged) {
        if (!wasInSync && Date.now() < this.awaitingPasteUntil) this.announceApplied = true;
        this.refreshLaunchStatuses();
      }
    } catch (e) {
      console.error("steamUserConfig failed", e);
    } finally {
      this.steamReadInFlight = false;
    }
  }

  /** The window regained focus. While a paste is expected but not yet seen,
   *  also re-check after 3 s and 10 s: Steam can write the file a moment after
   *  its Properties dialog closes. */
  onWindowFocus() {
    void this.refreshSteamConfig();
    // Back from lsfg-vk-ui with an edited profile list — pick it up.
    if (this.lsfgBuilderOpen) void this.refreshLsfgStatus();
    this.onWindowBlur();
    if (Date.now() < this.awaitingPasteUntil && this.syncState !== "in-sync") {
      this.followUps = [3_000, 10_000].map((ms) =>
        setTimeout(() => void this.refreshSteamConfig(true), ms),
      );
    }
  }

  /** Back to Steam (or elsewhere): stop the follow-up re-checks. */
  onWindowBlur() {
    for (const t of this.followUps) clearTimeout(t);
    this.followUps = [];
    void this.flushPersist();
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

  /** Every launch field back to its default for `game` — the options *and* the
   *  umu/runtime fields `resetOptions` leaves alone. No history entry; callers
   *  mark. A fresh game used to skip the umu/runtime half, so it inherited the
   *  previous game's exe, prefix and runtime, which then got saved under it. */
  private resetLaunchFields(game: GameDto | null) {
    this.resetOptions();
    const fresh = this.freshLaunch(game);
    this.umu = fresh.umu;
    this.umuExe = fresh.exe;
    this.umuWineprefix = fresh.prefix;
    this.umuGameid = "";
    this.selectedRuntime = fresh.runtime;
  }

  /** Whether `cfg` is exactly what opening `game` fresh would produce — i.e.
   *  the user hasn't tuned anything. umu fields only count in umu mode. */
  private isBaseline(cfg: Config, game: GameDto | null): boolean {
    const fresh = this.freshLaunch(game);
    if (cfg.env.length || cfg.wrappers.length || cfg.extra_env.trim() || cfg.game_args.trim()) {
      return false;
    }
    if (cfg.umu !== fresh.umu) return false;
    // A null runtime means "none chosen" — loadConfig falls back to the default.
    if (cfg.runtime !== null && cfg.runtime !== (fresh.runtime?.internal_name ?? null)) {
      return false;
    }
    return (
      !cfg.umu ||
      (cfg.umu_exe === fresh.exe &&
        cfg.umu_wineprefix.trim() === fresh.prefix &&
        !cfg.umu_gameid.trim())
    );
  }

  /** Save the selected game's config to `game_memory`, or drop its entry when
   *  it's untouched: merely opening a game used to save it, which put it under
   *  "Tuned" and suppressed the default-profile prompt for it forever. */
  private rememberCurrent() {
    if (this.selectedAppId == null) return;
    const key = String(this.selectedAppId);
    const cfg = this.toConfig();
    if (this.isBaseline(cfg, this.selectedGame)) delete this.store.game_memory[key];
    else this.store.game_memory[key] = cfg;
  }

  /** One-time repair of `game_memory` written before the fixes above: prune
   *  untouched entries, and give each Steam-mode entry back its own exe instead
   *  of the one it inherited from whichever game was open before it.
   *  Idempotent, so it simply runs on every load. */
  private repairGameMemory() {
    let changed = false;
    for (const [key, cfg] of Object.entries(this.store.game_memory)) {
      const game = this.games.find((g) => String(g.app_id) === key) ?? null;
      if (!cfg.umu) {
        const fresh = this.freshLaunch(game);
        if (cfg.umu_exe !== fresh.exe || cfg.umu_wineprefix !== fresh.prefix || cfg.umu_gameid) {
          cfg.umu_exe = fresh.exe;
          cfg.umu_wineprefix = fresh.prefix;
          cfg.umu_gameid = "";
          changed = true;
        }
      }
      if (this.isBaseline(cfg, game)) {
        delete this.store.game_memory[key];
        changed = true;
      }
    }
    if (changed) this.persistStore();
  }

  /**
   * A Nexus game replaces the Steam shortcut and Heroic sideload Nexus mirrors
   * it into, each of which had its own `game_memory` slot. Carry tuning,
   * favourite and last-open state from those ids over to the Nexus entry's —
   * preferring the mirror Nexus last imported from (first in `alias_ids`) —
   * so nothing tuned before is lost. Only copies: Nexus's own importer still
   * reads the old slots. An umu config gets the game's real prefix if it had
   * none, since that is what it always should have run in. Idempotent.
   */
  private adoptAliases() {
    let changed = false;
    for (const g of this.games) {
      if (!g.alias_ids?.length) continue;
      const key = String(g.app_id);
      if (!(key in this.store.game_memory)) {
        const from = g.alias_ids.find((a) => String(a) in this.store.game_memory);
        if (from != null) {
          const cfg = $state.snapshot(this.store.game_memory[String(from)]) as Config;
          if (cfg.umu && !cfg.umu_wineprefix.trim() && g.wine_prefix) cfg.umu_wineprefix = g.wine_prefix;
          this.store.game_memory[key] = cfg;
          changed = true;
        }
      }
      if (!this.store.favorites.includes(g.app_id) && g.alias_ids.some((a) => this.store.favorites.includes(a))) {
        this.store.favorites.push(g.app_id);
        changed = true;
      }
      if (this.store.last_game_appid != null && g.alias_ids.includes(this.store.last_game_appid)) {
        this.store.last_game_appid = g.app_id;
        changed = true;
      }
    }
    if (changed) this.persistStore();
  }

  /** Whether the selected game has saved tuning to forget. */
  get hasGameMemory(): boolean {
    return this.selectedAppId != null && String(this.selectedAppId) in this.store.game_memory;
  }

  /** Drop the selected game's saved tuning and reset it to a fresh open.
   *  Undoable like any other change. */
  forgetGameTuning() {
    if (this.selectedAppId == null) return;
    delete this.store.game_memory[String(this.selectedAppId)];
    this.resetLaunchFields(this.selectedGame);
    this.activePresetName = null;
    this.persistStore();
    this.mark(`forget tuning for ${this.selectedGameName ?? "this game"}`);
  }

  selectGame(game: GameDto | null) {
    // Persist the outgoing game's config.
    if (this.selectedAppId != null) {
      this.rememberCurrent();
      this.persistStore();
    }

    // Any pending prompt — or AI diagnosis, in flight or shown — belonged to
    // the game we're leaving.
    this.pendingDefaultPrompt = false;
    this.clearAnalysis();
    this.clearTroubleshoot();

    if (!game) {
      this.selectedAppId = null;
      this.selectedGameName = null;
      this.mark("switch to no game");
      return;
    }

    this.selectedAppId = game.app_id;
    this.selectedGameName = game.name;
    this.activePresetName = null;

    // Honour the "Auto-check ProtonDB" setting, which until now nothing read —
    // the toggle promised exactly this and did nothing. Steam apps only: a
    // shortcut's or Heroic game's appid is a synthetic hash protondb.com knows
    // nothing about. `requestTier` de-dupes per session, so re-opening a game
    // costs no request.
    if (this.store.protondb_auto && game.source === "steam") {
      lookups.requestTier(game.app_id);
    }
    lookups.requestAnticheat(game.app_id, this.store.anticheat_check);

    const remembered = this.store.game_memory[String(game.app_id)];
    if (remembered) {
      this.loadConfig(remembered);
    } else {
      this.resetLaunchFields(game);
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

  /** Nexus's key for the selected game, or null when it isn't a Nexus game —
   *  the gate for "Apply to Nexus" (and, by exclusion, for Heroic/Steam). */
  get nexusSlug(): string | null {
    const g = this.selectedGame;
    return g?.source === "nexus" ? (g.nexus_slug ?? null) : null;
  }

  /** Whether an Apply to Nexus is in flight, so the button can't double-fire. */
  nexusApplying = $state(false);

  /**
   * Hand the current tuning to Nexus as its launch profile for this game.
   * Nexus decodes the share code, saves it, and re-applies it to launch.sh
   * and the Steam/Heroic entries it keeps for the game — protongen never
   * writes those itself. The launch target is stripped by `encodePreset`;
   * Nexus supplies its own exe and prefix.
   */
  async applyToNexus() {
    const g = this.selectedGame;
    if (!g || this.nexusSlug == null || this.nexusApplying) return;
    this.nexusApplying = true;
    try {
      const code = encodePreset({ name: g.name.slice(0, 64), config: this.toConfig() });
      await ipc.applyToNexus(g.app_id, code);
      toast.success(`Applied to Nexus — ${g.name} launches with this tuning now`, { ms: 5000 });
    } catch (e) {
      toast.error(`Nexus didn't take it: ${e}`, { ms: 8000 });
    } finally {
      this.nexusApplying = false;
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

  /**
   * Merge `pendingVkBasaltSystemConfig` into the real, system-wide
   * `vkBasalt.conf`. Same shape as `exportMangoSystemWide` — backs the file up
   * first, preserves every line it doesn't own (deband/LUT tuning, ReShade
   * paths, comments), and clears a managed key the new config no longer sets.
   */
  async exportVkBasaltSystemWide() {
    try {
      const res = await ipc.exportVkbasaltSystem(this.pendingVkBasaltSystemConfig);
      const cleared = res.cleared_keys.length
        ? ` (cleared: ${res.cleared_keys.join(", ")})`
        : "";
      toast.success(`Set as vkBasalt config — backup saved${cleared}`, { ms: 6000 });
    } catch (e) {
      toast.error(`Couldn't write vkBasalt.conf: ${e}`, { ms: 6000 });
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

  presetExists(name: string): boolean {
    return this.store.presets.some((p) => p.name === name);
  }

  /** Exactly what `toConfig()` read right after each preset was loaded or saved
   *  this session, for `presetModified`. Keyed by name so undoing back onto a
   *  preset still compares against the right baseline. */
  private presetBaselines = new SvelteMap<string, string>();

  /** Whether the loaded preset has been edited since it was loaded or saved —
   *  the header shows a dot, so "Presets: X" never claims a stale match. */
  presetModified = $derived.by((): boolean => {
    const name = this.activePresetName;
    if (!name) return false;
    const now = JSON.stringify(withoutLaunchTarget(this.toConfig()));
    const baseline = this.presetBaselines.get(name);
    if (baseline !== undefined) return now !== baseline;
    const p = this.store.presets.find((x) => x.name === name);
    return !!p && now !== JSON.stringify(withoutLaunchTarget(p.config));
  });

  savePreset(name: string) {
    const config = withoutLaunchTarget(this.toConfig());
    const i = this.store.presets.findIndex((p) => p.name === name);
    // Overwriting keeps the preset's original game rather than silently
    // re-homing it onto whichever game happens to be open.
    const existing = i >= 0 ? this.store.presets[i] : null;
    const preset = {
      name,
      game_appid: existing ? existing.game_appid : this.selectedAppId,
      game_name: existing ? existing.game_name : this.selectedGameName,
      config,
    };
    if (i >= 0) this.store.presets[i] = preset;
    else this.store.presets.push(preset);
    this.activePresetName = name;
    this.presetBaselines.set(name, JSON.stringify(config));
    this.persistStore();
  }

  /** Save a preset received as a share code, optionally loading it too. */
  addSharedPreset(preset: { name: string; config: Config }, load: boolean) {
    const i = this.store.presets.findIndex((p) => p.name === preset.name);
    const entry = { name: preset.name, game_appid: null, game_name: null, config: preset.config };
    if (i >= 0) this.store.presets[i] = entry;
    else this.store.presets.push(entry);
    this.presetBaselines.delete(preset.name);
    this.persistStore();
    if (load) this.loadPreset(preset.name);
  }

  loadPreset(name: string) {
    const p = this.store.presets.find((x) => x.name === name);
    if (!p) return;
    this.loadConfig(p.config, { keepLaunchTarget: true });
    this.activePresetName = name;
    this.presetBaselines.set(name, JSON.stringify(withoutLaunchTarget(this.toConfig())));
    this.mark(`load preset "${name}"`);
  }

  /** Library multi-select, for applying one preset to many games. */
  selectMode = $state(false);
  selectedForBatch = new SvelteSet<number>();

  setSelectMode(on: boolean) {
    this.selectMode = on;
    if (!on) this.selectedForBatch.clear();
  }

  /**
   * Apply preset `name` to every game in `appIds` at once: each game's saved
   * tuning becomes the preset's, while its own launch target (umu mode, exe,
   * prefix, game id) is kept — or, for a game with nothing saved, set to what
   * opening it fresh would give. App state only; nothing is written to Steam,
   * Heroic or Nexus. Returns how many changed and an undo that puts every
   * touched entry back exactly (the builder's undo stack only covers the
   * open game).
   */
  applyPresetToGames(name: string, appIds: number[]): { count: number; undo: () => void } {
    const p = this.store.presets.find((x) => x.name === name);
    if (!p) return { count: 0, undo: () => {} };
    // Fold the open game's live edits into memory first, so they're what the
    // undo restores rather than a stale copy.
    if (this.selectedAppId != null) this.rememberCurrent();

    const before = new Map<string, Config | undefined>();
    const preset = $state.snapshot(p.config) as Config;
    for (const id of appIds) {
      const game = this.games.find((g) => g.app_id === id);
      if (!game) continue;
      const key = String(id);
      const prev = this.store.game_memory[key];
      before.set(key, prev ? ($state.snapshot(prev) as Config) : undefined);
      const fresh = this.freshLaunch(game);
      this.store.game_memory[key] = {
        ...structuredClone(preset),
        umu: prev ? prev.umu : fresh.umu,
        umu_exe: prev ? prev.umu_exe : fresh.exe,
        umu_wineprefix: prev ? prev.umu_wineprefix : fresh.prefix,
        umu_gameid: prev ? prev.umu_gameid : "",
        runtime: preset.runtime ?? prev?.runtime ?? fresh.runtime?.internal_name ?? null,
      };
    }

    const reloadOpen = () => {
      const open = this.selectedAppId;
      if (open == null || !before.has(String(open))) return;
      const cfg = this.store.game_memory[String(open)];
      if (cfg) this.loadConfig(cfg);
      else this.resetLaunchFields(this.selectedGame);
      this.mark(`apply preset "${name}" to ${before.size} games`);
    };
    reloadOpen();
    this.persistStore();
    this.refreshLaunchStatuses();

    return {
      count: before.size,
      undo: () => {
        for (const [key, cfg] of before) {
          if (cfg) this.store.game_memory[key] = cfg;
          else delete this.store.game_memory[key];
        }
        reloadOpen();
        this.persistStore();
        this.refreshLaunchStatuses();
      },
    };
  }

  /** Rename a preset. Returns why it couldn't, or null on success. */
  renamePreset(from: string, to: string): string | null {
    const name = to.trim();
    if (!name) return "A preset needs a name.";
    if (name === from) return null;
    if (this.presetExists(name)) return `There's already a preset called “${name}”.`;
    const p = this.store.presets.find((x) => x.name === from);
    if (!p) return null;
    p.name = name;
    const baseline = this.presetBaselines.get(from);
    this.presetBaselines.delete(from);
    if (baseline !== undefined) this.presetBaselines.set(name, baseline);
    if (this.activePresetName === from) this.activePresetName = name;
    this.persistStore();
    return null;
  }

  /** Delete a preset, returning what `restorePreset` needs to undo it. */
  deletePreset(name: string): { preset: Preset; index: number } | null {
    const index = this.store.presets.findIndex((p) => p.name === name);
    if (index < 0) return null;
    const preset = $state.snapshot(this.store.presets[index]) as Preset;
    this.store.presets.splice(index, 1);
    if (this.activePresetName === name) this.activePresetName = null;
    this.persistStore();
    return { preset, index };
  }

  /** Put a deleted preset back where it was — unless its name was taken since. */
  restorePreset(preset: Preset, index: number) {
    if (this.presetExists(preset.name)) return;
    this.store.presets.splice(Math.min(index, this.store.presets.length), 0, preset);
    this.persistStore();
  }

  // --------------------------- global profile -------------------------------

  /** Save the current build as the reusable global profile (Settings). */
  setGlobalProfileFromCurrent() {
    this.store.global_profile = withoutLaunchTarget(this.toConfig());
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
    this.loadConfig(gp, { keepLaunchTarget: true });
    this.mark("apply global profile");
  }

  // ------------------------------- import -----------------------------------

  /** Replace the current config with a parsed launch command. Throws — rather
   *  than wiping the config — when the text isn't recognisably a command.
   *  Returns the tokens that couldn't be imported, for the caller to report. */
  async importCommand(text: string): Promise<string[]> {
    const { config: cfg, dropped } = await ipc.parseCommand(text);
    const empty =
      !cfg.env.length &&
      !cfg.wrappers.length &&
      !cfg.extra_env.trim() &&
      !cfg.game_args.trim() &&
      !cfg.umu_exe.trim();
    if (empty && !dropped.length && !/%command%|umu-run/.test(text)) {
      throw new Error(
        "Nothing recognisable — paste a Steam launch-options string or a umu-run command.",
      );
    }
    this.loadConfig(cfg);
    this.mark("import command");
    return dropped;
  }

  // ------------------------------- mangohud ---------------------------------

  /** Set a catalog env row to `value` (enabled; "" turns it off), or — when
   *  the catalog lacks the key — set it in the custom-env field instead, with
   *  the builder's quoting and replacing any earlier assignment. */
  private setEnvOrExtra(key: string, value: string) {
    const s = this.env[key];
    if (s) {
      s.enabled = value !== "";
      s.value = value;
      this.disownParam(key);
    } else if (value) {
      this.extraEnv = setInExtraEnv(this.extraEnv, key, value);
    }
  }

  applyMango(config: string) {
    this.setEnvOrExtra("MANGOHUD_CONFIG", config);
    // The in-app string shouldn't compete with a stale config-file path.
    this.applyEnv("MANGOHUD_CONFIGFILE", false);
    if (this.wrap["mangohud"]) this.wrap["mangohud"].enabled = true;
    this.mark("apply MangoHud preset");
  }

  applyMangoFile(path: string) {
    this.setEnvOrExtra("MANGOHUD_CONFIGFILE", path);
    // MANGOHUD_CONFIG takes priority over config files — disable it so the
    // file's settings actually take effect.
    this.applyEnv("MANGOHUD_CONFIG", false);
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
    this.setEnvOrExtra("PROTON_OPTISCALER_CONFIG", config);
    this.setEnvOrExtra("PROTON_OPTISCALER_NAME", proxy);
    this.applyEnv("PROTON_USE_OPTISCALER", true);
    this.mark("apply OptiScaler config");
  }

  // --------------------------- lossless scaling -----------------------------

  /** lsfg-vk on this machine (layer, conf.toml profiles, Lossless.dll); `null`
   *  until the first read lands. */
  lsfgStatus = $state<LsfgStatus | null>(null);
  lsfgStatusLoading = $state(false);
  /** Why the last read failed; cleared by the next successful one. Without it a
   *  failed first read left the builder on "Looking for lsfg-vk…" forever. */
  lsfgStatusError = $state<string | null>(null);

  /** (Re-)read lsfg-vk's state. Cheap and read-only, so it runs on every open
   *  of the builder and on window focus while it's open — profiles are edited
   *  in lsfg-vk-ui, and a stale list would offer a name that no longer exists. */
  async refreshLsfgStatus() {
    if (this.lsfgStatusLoading) return;
    this.lsfgStatusLoading = true;
    try {
      this.lsfgStatus = await ipc.lsfgStatus();
      this.lsfgStatusError = null;
    } catch (e) {
      console.error("lsfgStatus failed", e);
      this.lsfgStatusError = String(e);
    } finally {
      this.lsfgStatusLoading = false;
    }
  }

  /** Lossless Scaling frame generation is switched on in this launch string
   *  (a profile, or per-game settings, and not vetoed). */
  get lsfgActive(): boolean {
    const on = (k: string) => this.env[k]?.enabled === true;
    return !on("DISABLE_LSFGVK") && (on("LSFGVK_PROFILE") || on("LSFGVK_ENV"));
  }

  /**
   * Apply the Lossless Scaling builder as one undo step: every builder-owned
   * variable off, then exactly `pairs` on — so switching between a profile and
   * per-game settings never leaves the other mode's variables behind.
   */
  applyLsfg(pairs: [string, string][], label = "apply Lossless Scaling") {
    const keep = new Set(pairs.map(([k]) => k));
    for (const key of LSFG_BUILDER_KEYS) if (!keep.has(key)) this.applyEnv(key, false);
    for (const [key, value] of pairs) this.setEnvOrExtra(key, value);
    this.mark(label);
  }

  /** CachyOS Proton injects its own OptiScaler (from the prefix) at launch:
   *  `PROTON_USE_OPTISCALER` on and not `0` — `ipc::lint`'s same test. */
  get optiInjected(): boolean {
    const row = this.env["PROTON_USE_OPTISCALER"];
    return !!row?.enabled && row.value.trim() !== "0";
  }

  /** OptiScaler's own frame generation (OptiFG) is on in its inline config. */
  get optiFgOn(): boolean {
    const row = this.env["PROTON_OPTISCALER_CONFIG"];
    return !!row?.enabled && parseOptiScaler(row.value).frameGenOn;
  }

  /**
   * Keep OptiScaler for upscaling but turn its frame generation off — the
   * pairing with Lossless Scaling. Same result as `lint.rs`'s
   * `lsfg-double-framegen` fix: an explicit `FrameGen.Enabled=false`.
   */
  disableOptiFg() {
    const row = this.env["PROTON_OPTISCALER_CONFIG"];
    if (!row) return;
    const c = parseOptiScaler(row.value);
    c.frameGenOn = false;
    const rest = buildOptiScaler(c);
    this.setEnvOrExtra("PROTON_OPTISCALER_CONFIG", [rest, "FrameGen.Enabled=false"].filter(Boolean).join(";"));
    this.mark("turn off OptiScaler frame generation");
  }

  /** Inject OptiScaler (upscaling) alongside Lossless Scaling. */
  enableOptiScaler() {
    this.applyEnv("PROTON_USE_OPTISCALER", true);
    this.mark("enable OptiScaler");
  }

  /** Swap the Lossless Scaling builder for the OptiScaler one. Sequential, never
   *  stacked: the second modal opens once the first has finished closing (#63). */
  openOptiFromLsfg() {
    this.lsfgBuilderOpen = false;
    setTimeout(() => (this.optiBuilderOpen = true), 250);
  }

  /** The reverse hop, from the OptiScaler builder's frame-generation section. */
  openLsfgFromOpti() {
    this.optiBuilderOpen = false;
    setTimeout(() => (this.lsfgBuilderOpen = true), 250);
  }

  // ------------------------------- recipes ----------------------------------

  /** Apply a recipe as one undoable step. Owns its feedback (success toast with
   *  Undo, or an error toast), so every call site reports the same way. */
  async applyRecipe(index: number): Promise<boolean> {
    const recipe = this.recipes[index];
    const name = recipe?.name ?? "recipe";
    // A double click would merge twice against the same stale base.
    if (this.applyingRecipe) return false;
    this.applyingRecipe = true;
    const appId = this.selectedAppId;
    const base = this.toConfig();
    let cfg: Config;
    try {
      cfg = await ipc.applyRecipe(index, base);
    } catch (e) {
      console.error("applyRecipe failed", e);
      toast.error(`Couldn't apply “${name}”: ${e}`);
      return false;
    } finally {
      this.applyingRecipe = false;
    }
    // The result was merged onto the config of the game that was open when the
    // request went out; landing it on a game opened since would overwrite that one.
    if (this.selectedAppId !== appId) return false;
    // Same for edits made while it was in flight: loadConfig would erase them.
    if (JSON.stringify(this.toConfig()) !== JSON.stringify(base)) {
      toast.info(`Didn't apply “${name}” — the build changed while it was loading. Try again.`);
      return false;
    }
    this.loadConfig(cfg);
    // loadConfig → resetOptions clears the map, so attribute after, not before.
    if (recipe) {
      for (const [key] of [...recipe.env, ...recipe.wrappers]) {
        this.recipeOrigin[key] = recipe.name;
      }
    }
    // The most destructive action in the app: recipes.rs is additive-only, so
    // stacking them accumulates with no way back short of a reset.
    this.mark(`apply "${name}"`);
    toast.success(`Applied: ${name}`, { action: { label: "Undo", onClick: () => this.undo() } });
    return true;
  }

  // ------------------------------- theme/store ------------------------------

  setTheme(id: string) {
    this.store.theme = id;
    applyTheme(id);
    this.persistStore();
  }

  /** Where a Simple-mode jump should scroll to (see `setSection`). */
  simpleAnchor = $state<{ id: string; nonce: number } | null>(null);
  /** Whether the Recipes card is collapsed. On the store rather than in the
   *  component so it survives Recipes unmounting (switching sections) and so a
   *  jump to it can open it. Per session, like the Settings sections. */
  recipesCollapsed = $state(true);

  /**
   * Navigate to a section, from anywhere: the library, Simple or Advanced mode.
   * Palette actions, notice chips and ActiveOptions links all route through
   * here, and used to do nothing outside the Advanced builder — the section
   * they set only exists in Advanced's MainPanel.
   *
   * In Simple mode, the three sections Simple also shows scroll into view there;
   * anything else switches to Advanced (with a way back).
   */
  setSection(section: string) {
    this.view = "builder";
    this.paramQuery = "";
    this.activeSection = section;
    if (section === "recipes") this.recipesCollapsed = false;
    if (this.uiMode !== "simple") return;
    const anchor = SIMPLE_ANCHORS[section];
    if (anchor) this.simpleAnchor = { id: anchor, nonce: ++this.focusNonce };
    else this.leaveSimpleFor(section);
  }

  /** Switch to Advanced to show something Simple mode has no room for, with a
   *  one-click way back — the user didn't ask to change modes, only to go there. */
  private leaveSimpleFor(what: string) {
    this.setUiMode("advanced");
    toast.info(`Switched to Advanced to show ${what}`, {
      action: { label: "Back to Simple", onClick: () => this.setUiMode("simple") },
    });
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
  /** Same as `heroicConfirmOpen`, for "Apply to Nexus?" (`NexusConfirm`). */
  nexusConfirmOpen = $state(false);

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

  /** Same shape again, for the vkBasalt builder's "Set as vkBasalt config?"
   *  confirmation — vkBasalt has no inline env-var carrier, so this is the
   *  builder's *only* apply action, not a second one alongside a command-apply
   *  path like MangoHud/OptiScaler have. */
  vkSystemConfirmOpen = $state(false);
  pendingVkBasaltSystemConfig = $state("");

  /**
   * Whether the MangoHud / OptiScaler / vkBasalt overlay-builder dialogs are up.
   *
   * Lives on the store for the same reason as `heroicConfirmOpen`: SimplePanel
   * and MainPanel each used to own a local `$state` + `<Dialog>` pair for these,
   * and switching UI mode (the Simple/Advanced toggle) unmounts whichever panel
   * is showing — the #63 failure mode again, a bits-ui modal torn out while
   * open, stranding `body { pointer-events: none }` and bricking every click.
   * The dialogs now mount once, at the app root (`OverlayBuilders`).
   */
  mangoBuilderOpen = $state(false);
  optiBuilderOpen = $state(false);
  vkBuilderOpen = $state(false);
  /** The gamescope builder (`GamescopeBuilder`, root-mounted in OverlayBuilders). */
  gamescopeBuilderOpen = $state(false);
  /** The WINEDLLOVERRIDES editor (`DllOverrides`, root-mounted in OverlayBuilders). */
  dllBuilderOpen = $state(false);
  /** The Compare dialog (`CompareDialog`, root-mounted in App.svelte). */
  compareOpen = $state(false);
  lsfgBuilderOpen = $state(false);

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

    // Named after the key rather than its category — that's what was asked for.
    if (this.uiMode === "simple") this.leaveSimpleFor(key);
    this.setSection(env ? env.category : "Wrappers");
    this.focusParam = { key, nonce: ++this.focusNonce };
    return true;
  }

  /**
   * The AMD generation in force: the user's Settings declaration if they made
   * one, else what `hardware.rs` detected from the PCI id.
   *
   * The declaration wins outright. Detection is best-effort — it needs hwdata's
   * `pci.ids` on disk and a `Navi <n>` codename in the entry — so it fills a
   * gap, it never overrules someone who has said what they have. Mirrored on the
   * Rust side in `lint::effective_gpu_gen` (with `hwCaps`'s AMD gate below), or
   * the lint rules and this filter would disagree about which generation is in
   * force.
   */
  effectiveGpuGen = $derived(this.store.gpu_gen || (this.hardware.gpu_gen_detected ?? ""));

  /**
   * Hardware facts plus the opt-in HDR/FSR/GPU-generation capabilities, for
   * relevance filtering. `fsr4` is true for either RDNA generation (with the
   * legacy `store.fsr4` flag as a fallback for pre-`gpu_gen` state files);
   * `rdna3` and `rdna4` are exclusive, so each generation's options hide on the
   * other.
   *
   * Both generation flags are gated on `hardware.amd`. The generation itself is
   * a persisted free string that nothing re-validates against the detected GPU,
   * so a state file carried to an NVIDIA machine would otherwise keep unlocking
   * AMD-only rows — including `PROTON_FSR4_INDICATOR`, which had no `gpu` hint of
   * its own.
   *
   * `$derived` rather than a getter because it allocates: every consumer calls
   * `irrelevance(app.hwCaps, …)` once *per row*, so as a getter this built a
   * fresh object 100+ times per render of the parameter list and again for every
   * entry in the command palette.
   */
  hwCaps = $derived.by((): HwCaps => {
    const gen = this.effectiveGpuGen;
    const amd = this.hardware.amd;
    return {
      ...this.hardware,
      hdr: this.store.hdr,
      fsr4: amd && (gen === "rdna3" || gen === "rdna4" || this.store.fsr4),
      rdna3: amd && gen === "rdna3",
      rdna4: amd && gen === "rdna4",
    };
  });

  /** Catalog env keys tagged as a good default for the current GPU
   *  capabilities that aren't already on at their recommended value — what
   *  `applyRecommendedForGpu` would still change. Empty means the button has
   *  nothing to do (either untagged hardware, or already applied). */
  recommendedEnvKeys = $derived.by((): string[] => {
    const caps = this.hwCaps;
    return this.catalog.envs
      .filter((d) => isRecommended(caps, d.recommended_for))
      .filter((d) => {
        const s = this.env[d.key];
        if (!s) return false;
        return !s.enabled || (d.default_value !== "" && s.value !== d.default_value);
      })
      .map((d) => d.key);
  });

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

  /** Open Settings, optionally expanded and scrolled to one section. */
  openSettings(section?: SettingsSection) {
    if (section) {
      this.settingsSections[section] = true;
      this.settingsFocus = { section, nonce: ++this.focusNonce };
    }
    this.showSettings = true;
  }

  /** Close Settings, then replay the intro tour once the drawer has finished
   *  sliding out — two modal layers must never overlap (#63). */
  replayTour() {
    this.showSettings = false;
    setTimeout(() => this.tourReplay++, 250);
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
    this.persistStoreSoon();
  }
  setLlmModel(v: string) {
    this.store.llm_model = v;
    this.persistStoreSoon();
  }

  // ------------------------------ paths -------------------------------------

  /** Replace one of the path lists and re-scan. */
  setPathList(field: "steam_roots" | "steam_libraries" | "proton_dirs", list: string[]) {
    this.store.paths[field] = list;
    this.persistStoreSoon();
    this.scheduleRescan();
  }

  /** Override the program token emitted for `name` ("" clears the override). */
  setBinOverride(name: string, value: string) {
    if (value.trim() === "") delete this.store.paths.bins[name];
    else this.store.paths.bins[name] = value;
    this.persistStoreSoon();
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
    // The scan reads paths off the backend's store, so the save goes first.
    this.rescanTimer = setTimeout(() => void this.flushPersist().finally(() => this.refresh()), 600);
  }

  /** Path warnings only — the parse warnings belong to the TOML overrides. */
  get pathWarnings() {
    return this.configWarnings.filter((w) => w.kind === "path");
  }

  // ------------------------------ library view ------------------------------

  /** `favorites` is a list (mirroring the Rust `BTreeSet`), so membership was an
   *  O(n) `includes`. The library's sort comparator calls `isFavorite` twice per
   *  comparison, i.e. O(n log n) times per keystroke. */
  private favoriteSet = $derived(new Set(this.store.favorites));

  isFavorite(appId: number): boolean {
    return this.favoriteSet.has(appId);
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

  // ------------------------------ anti-cheat --------------------------------

  setAnticheatCheck(v: boolean) {
    this.store.anticheat_check = v;
    this.persistStore();
    if (v && this.selectedAppId != null) lookups.requestAnticheat(this.selectedAppId, true);
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
        this.optiscalerStatusCache[String(appId)] = {
          install_dir: null,
          found: false,
          proxies: [],
          stray_dll: false,
        };
      })
      .finally(() => (this.optiscalerStatusLoading[String(appId)] = false));
  }

  /** Which build line the upgrade panel fetches. Session-only on purpose:
   *  nightly is a per-fetch opt-in, so every launch starts back on stable. */
  optiscalerChannel = $state<OptiscalerChannel>("stable");

  /** The latest upstream OptiScaler release per channel — global, not
   *  per-game, so each is fetched at most once per session regardless of which
   *  game is open. */
  private optiscalerLatestBy = $state<Partial<Record<OptiscalerChannel, OptiscalerRelease>>>({});
  private optiscalerLatestLoadingBy = $state<Partial<Record<OptiscalerChannel, boolean>>>({});
  private optiscalerLatestErrorBy = $state<Partial<Record<OptiscalerChannel, string>>>({});
  private optiscalerLatestRequested = new Set<OptiscalerChannel>();

  get optiscalerLatest(): OptiscalerRelease | null {
    return this.optiscalerLatestBy[this.optiscalerChannel] ?? null;
  }
  get optiscalerLatestLoading(): boolean {
    return this.optiscalerLatestLoadingBy[this.optiscalerChannel] === true;
  }
  get optiscalerLatestError(): string | null {
    return this.optiscalerLatestErrorBy[this.optiscalerChannel] ?? null;
  }

  setOptiscalerChannel(channel: OptiscalerChannel) {
    this.optiscalerChannel = channel;
    this.requestOptiscalerLatest();
  }

  /** For the current channel; once per channel per session. */
  requestOptiscalerLatest() {
    const channel = this.optiscalerChannel;
    if (this.optiscalerLatestRequested.has(channel)) return;
    this.optiscalerLatestRequested.add(channel);
    this.optiscalerLatestLoadingBy[channel] = true;
    delete this.optiscalerLatestErrorBy[channel];
    ipc
      .optiscalerLatest(channel)
      .then((r) => (this.optiscalerLatestBy[channel] = r))
      .catch((e) => (this.optiscalerLatestErrorBy[channel] = String(e)))
      .finally(() => (this.optiscalerLatestLoadingBy[channel] = false));
  }

  optiscalerFetchBusy = $state(false);

  /** Download the latest OptiScaler release and extract it into `appId`'s
   *  install directory. The one action in the app that writes into a game's
   *  own folder — callers gate this behind an explicit confirm, never call it
   *  from a $effect or on load. Throws on failure; caller toasts. */
  async fetchOptiscalerUpgrade(appId: number): Promise<OptiscalerExtractResult> {
    this.optiscalerFetchBusy = true;
    try {
      const result = await ipc.optiscalerFetch(appId, this.optiscalerChannel);
      const prev = this.optiscalerStatusCache[String(appId)];
      this.optiscalerStatusCache[String(appId)] = {
        install_dir: prev?.install_dir ?? null,
        found: true,
        proxies: prev?.proxies ?? [],
        stray_dll: prev?.stray_dll ?? false,
      };
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
    const seq = ++this.aiSeq;
    this.aiLoading = true;
    this.aiError = null;
    try {
      const result = await ipc.llmAnalyze({
        command: this.command,
        game_name: this.selectedGameName ?? "",
        error_lines: log.error_lines,
        log_tail: log.tail,
      });
      if (seq === this.aiSeq) this.aiResult = result;
    } catch (e) {
      console.error("llmAnalyze failed", e);
      if (seq !== this.aiSeq) return;
      this.aiError = String(e);
      this.aiResult = null;
    } finally {
      if (seq === this.aiSeq) this.aiLoading = false;
    }
  }

  /** Bumped per request and by the clear methods: a response only lands if no
   *  newer request or clear happened while it was in flight — otherwise a
   *  slow answer for the game (or dialog) you left shows up afterwards. */
  private aiSeq = 0;
  private tsSeq = 0;

  /** Clear the last analysis (e.g. when the log dialog closes or the game
   *  changes) so a stale suggestion never shows against a different game. */
  clearAnalysis() {
    this.aiSeq++;
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
    const seq = ++this.tsSeq;
    this.tsLoading = true;
    this.tsError = null;
    try {
      let errorLines: string[] = [];
      let hasLog = false;
      if (this.selectedAppId != null) {
        try {
          const log = await ipc.readProtonLog(this.selectedAppId, this.toConfig());
          if (log.present) {
            hasLog = true;
            errorLines = log.error_lines;
          }
        } catch {
          // The log is optional context; a read failure must not block the
          // symptom-only diagnosis.
        }
      }
      const result = await ipc.llmTroubleshoot({
        symptom,
        command: this.command,
        game_name: this.selectedGameName ?? "",
        error_lines: errorLines,
        has_log: hasLog,
      });
      if (seq === this.tsSeq) this.tsResult = result;
    } catch (e) {
      console.error("llmTroubleshoot failed", e);
      if (seq !== this.tsSeq) return;
      this.tsError = String(e);
      this.tsResult = null;
    } finally {
      if (seq === this.tsSeq) this.tsLoading = false;
    }
  }

  /** Reset the troubleshooter (e.g. when its dialog closes). */
  clearTroubleshoot() {
    this.tsSeq++;
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
    if (!this.env[change.key] && !this.wrap[change.key]) return false;
    this.applyChanges(`AI: set ${change.key}`, { enable: [[change.key, change.value]] });
    return true;
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

  async checkRuntimeUpdates() {
    try {
      this.runtimeUpdates = await ipc.checkRuntimeUpdates();
    } catch (e) {
      console.error("runtime update check failed", e);
    }
  }

  get visibleRuntimeUpdates(): RuntimeUpdate[] {
    const dismissed = this.store.dismissed_runtime_updates ?? [];
    return this.runtimeUpdates.filter((u) => !dismissed.includes(u.tag));
  }

  dismissRuntimeUpdate(tag: string) {
    const prev = this.store.dismissed_runtime_updates ?? [];
    if (prev.includes(tag)) return;
    // Keep only tags still on offer, so the list can't grow forever.
    const live = new Set(this.runtimeUpdates.map((u) => u.tag));
    this.store.dismissed_runtime_updates = [...prev.filter((t) => live.has(t)), tag];
    this.persistStore();
  }

  /** Download, verify and swap the new binary. On success the backend restarts
   *  the app into the new version, so this never returns in the real shell. */
  async applyUpdate() {
    if (!this.update) return;
    this.updating = true;
    try {
      await ipc.runUpdate();
    } finally {
      this.updating = false;
    }
  }

  /** The payload `save_store` last accepted, so an unchanged store is not
   *  re-sent, re-serialized to TOML and re-written to disk. */
  private lastPersisted: string | null = null;

  private persistTimer: ReturnType<typeof setTimeout> | undefined;

  /** `persistStore`, debounced — for setters wired to `oninput`, where every
   *  keystroke would otherwise be a full `save_store` IPC plus a TOML write. */
  persistStoreSoon() {
    clearTimeout(this.persistTimer);
    this.persistTimer = setTimeout(() => void this.persistStore(), 400);
  }

  /** Write a debounced save now, if one is pending (before a rescan reads the
   *  store, or the window closes). */
  flushPersist(): Promise<void> {
    return this.persistTimer === undefined ? Promise.resolve() : this.persistStore();
  }

  persistStore(): Promise<void> {
    // Saves everything, so it supersedes any debounced save still pending.
    clearTimeout(this.persistTimer);
    this.persistTimer = undefined;
    const payload = $state.snapshot(this.store);
    const serialized = JSON.stringify(payload);
    // `save_store` replaces the file wholesale (#43), so an identical payload is
    // a genuine no-op — and this runs on a 500 ms debounce while the user types,
    // where most ticks change nothing the store holds. Skipped only while writes
    // are working: a standing failure banner has to be able to clear, and the
    // retry is this same call.
    if (serialized === this.lastPersisted && !this.persistError) return Promise.resolve();

    // Fire and forget for most callers; the store is small.
    return ipc
      .saveStore(payload)
      .then(() => {
        this.lastPersisted = serialized;
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

export type SettingsSection = "appearance" | "behavior" | "ai" | "paths" | "about";

/** Sections Simple mode also shows, and the element id each scrolls to. */
const SIMPLE_ANCHORS: Record<string, string> = {
  recipes: "simple-recipes",
  "@active": "simple-active",
  game: "simple-game",
};

/** Same keys, same values — so an unchanged re-read doesn't churn reactivity. */
function sameEntries(a: Record<string, string>, b: Record<string, string>): boolean {
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every((k) => a[k] === b[k]);
}

export const app = new AppStore();
