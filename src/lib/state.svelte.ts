import { ipc } from "./ipc";
import { toast } from "./toast.svelte";
import { applyTheme, DEFAULT_THEME } from "./themes";
import { emptyConfig } from "./types";
import type {
  Catalog,
  Config,
  DiffStatus,
  GameDto,
  Hardware,
  Notice,
  Recipe,
  RuntimeDto,
  ConfigWarning,
  StaleInfo,
  Store,
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
  hdr: false,
  fsr4: false,
  protondb_auto: false,
  last_session: null,
  last_game_appid: null,
};

export type ArtKind = "portrait" | "hero" | "header";

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

  // ---- game art (lazy, cached): key `${source}:${appId}:${kind}` ----
  //   undefined = not requested/loading · null = none found · string = data URL
  artCache = $state<Record<string, string | null>>({});
  private artRequested = new Set<string>();

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

  /** Re-scan the library (games, runtimes, shortcuts) without restarting. Only
   *  the discovery-derived fields are replaced; builder selections, the store and
   *  the theme are left intact. Current selections are re-validated against the
   *  fresh lists so a removed game/runtime falls back gracefully. */
  async refresh() {
    if (this.refreshing) return;
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
    } finally {
      this.refreshing = false;
    }
    // launchOptions just changed, so every badge is potentially stale.
    this.refreshLaunchStatuses();
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
  }

  toggleEnv(key: string) {
    const s = this.env[key];
    if (s) s.enabled = !s.enabled;
  }
  setEnvValue(key: string, value: string) {
    const s = this.env[key];
    if (s) s.value = value;
  }
  toggleWrap(key: string) {
    const s = this.wrap[key];
    if (s) s.enabled = !s.enabled;
  }
  setWrapValue(key: string, value: string) {
    const s = this.wrap[key];
    if (s) s.value = value;
  }

  enabledCountInCategory(category: string): number {
    return this.catalog.envs.filter(
      (e) => e.category === category && this.env[e.key]?.enabled,
    ).length;
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
    for (const [k, v] of cfg.env) {
      if (this.env[k]) this.env[k] = { enabled: true, value: v };
    }
    for (const [k, v] of cfg.wrappers) {
      if (this.wrap[k]) this.wrap[k] = { enabled: true, value: v };
    }
    this.umu = cfg.umu;
    this.extraEnv = cfg.extra_env;
    this.gameArgs = cfg.game_args;
    this.umuExe = cfg.umu_exe;
    this.umuWineprefix = cfg.umu_wineprefix;
    this.umuGameid = cfg.umu_gameid;
    if (cfg.runtime) {
      const r = this.runtimes.find((x) => x.internal_name === cfg.runtime);
      if (r) this.selectedRuntime = r;
    }
  }

  /** Reset the command back to defaults, keeping the selected game. Returns the
   *  prior config so the caller can offer an undo. */
  resetCommand(): Config {
    const prev = this.toConfig();
    this.resetOptions();
    this.umu = false;
    this.umuExe = "";
    this.umuWineprefix = "";
    this.umuGameid = "";
    this.activePresetName = null;
    this.selectedRuntime = this.defaultRuntime();
    return prev;
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
      try {
        const command = await ipc.buildCommand(cfg, path);
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

      try {
        const notices = await ipc.lint(cfg);
        if (seq === this.recomputeSeq) this.notices = notices;
      } catch (e) {
        console.error("lint failed", e);
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

  // ------------------------------- game memory ------------------------------

  selectGame(game: GameDto | null) {
    // Persist the outgoing game's config.
    if (this.selectedAppId != null) {
      this.store.game_memory[String(this.selectedAppId)] = this.toConfig();
      this.persistStore();
    }

    if (!game) {
      this.selectedAppId = null;
      this.selectedGameName = null;
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
    }
  }

  // ------------------------------- presets ----------------------------------

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
  }

  deletePreset(name: string) {
    this.store.presets = this.store.presets.filter((p) => p.name !== name);
    if (this.activePresetName === name) this.activePresetName = null;
    this.persistStore();
  }

  // ------------------------------- import -----------------------------------

  async importCommand(text: string) {
    const cfg = await ipc.parseCommand(text);
    this.loadConfig(cfg);
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
  }

  // ------------------------------- recipes ----------------------------------

  async applyRecipe(index: number) {
    const cfg = await ipc.applyRecipe(index, this.toConfig());
    this.loadConfig(cfg);
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

  /** Hardware facts plus the opt-in HDR/FSR capabilities, for relevance filtering. */
  get hwCaps() {
    return { ...this.hardware, hdr: this.store.hdr, fsr4: this.store.fsr4 };
  }

  setShowIrrelevant(v: boolean) {
    this.store.show_irrelevant = v;
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
  setProtondbAuto(v: boolean) {
    this.store.protondb_auto = v;
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

  /** Lazily fetch a game's art once; result lands in `artCache` reactively. */
  requestArt(appId: number, source: string, kind: ArtKind) {
    const key = this.artKey(appId, source, kind);
    if (this.artRequested.has(key)) return;
    this.artRequested.add(key);
    // "Local + online fallback" per the user's choice: allow the CDN backstop.
    ipc
      .gameArt(appId, source, kind, true)
      .then((url) => (this.artCache[key] = url))
      .catch(() => (this.artCache[key] = null));
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
