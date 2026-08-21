// Mirrors the serde DTOs exposed by src-tauri/src/ipc.rs.

export type WrapperKind = "plain" | "gamescope";
export type RecipeKind = "profile" | "fix";
export type Pair = [string, string];

export interface WrapperDef {
  key: string;
  label: string | null;
  kind: WrapperKind;
  default_value: string;
  requires: string | null;
  help: string;
  details: string | null;
  example: string | null;
  url: string | null;
  gpu: string | null;
  needs: string[];
  /** `"advanced"` hides this behind the show-advanced toggle; anything else
   *  (including the `""` an entry without the field produces) is basic. */
  tier: string;
}

export interface EnvDef {
  key: string;
  category: string;
  default_value: string;
  values: string[];
  requires: string | null;
  help: string;
  details: string | null;
  example: string | null;
  url: string | null;
  gpu: string | null;
  needs: string[];
  /** `"advanced"` hides this behind the show-advanced toggle; anything else
   *  (including the `""` an entry without the field produces) is basic. */
  tier: string;
}

/**
 * True when an entry should stay hidden behind the "show advanced" toggle.
 *
 * A predicate rather than a bare `=== "advanced"` at each call site so the
 * default direction is stated once: anything unrecognised — including the empty
 * string a pre-`tier` catalog or a user's own `params.toml` override produces —
 * is basic, i.e. visible. Mirrors `params::TIER_ADVANCED`.
 */
export function isAdvanced(def: { tier: string }): boolean {
  return def.tier === "advanced";
}

export interface Meta {
  proton_cachyos_build: string | null;
  updated: string | null;
}

export interface Catalog {
  meta: Meta;
  wrappers: WrapperDef[];
  envs: EnvDef[];
}

export interface Recipe {
  name: string;
  kind: RecipeKind;
  description: string;
  symptom: string | null;
  gpu: string | null;
  needs: string[];
  icon: string | null;
  accent: string | null;
  tags: string[];
  env: Pair[];
  wrappers: Pair[];
}

export interface Hardware {
  nvidia: boolean;
  amd: boolean;
  intel: boolean;
  wayland: boolean;
  kde: boolean;
  ntsync: boolean;
}

/** The AMD generation the user declared in Settings; "" when unset or not AMD.
 *  Not auto-detectable — `/sys/module/amdgpu` is loaded for everything from GCN
 *  onwards, and nothing in the app reads PCI ids. */
export type GpuGen = "" | "rdna3" | "rdna4";

/** UI density mode. Mirrors `store::Store.ui_mode` (a Rust `String`); the
 *  frontend treats anything unrecognised — including the `""` an older
 *  `state.toml` carries — as `"simple"` via `app.uiMode`. */
export type UiMode = "simple" | "advanced";

/**
 * The `needs` tags a catalog entry or recipe may declare. Kept in lockstep with
 * the branches in `util.ts irrelevance()` and with `KNOWN_NEEDS` in
 * `src-tauri/src/params.rs`, which asserts the shipped TOML only uses these.
 */
export type Capability = "wayland" | "kde" | "ntsync" | "hdr" | "fsr4" | "rdna3" | "rdna4";

/** Detected hardware plus the opt-in capabilities, as `irrelevance()` sees it. */
export type HwCaps = Hardware & Record<Exclude<Capability, "wayland" | "kde" | "ntsync">, boolean>;

export interface RuntimeDto {
  internal_name: string;
  display_name: string;
  kind: string; // "system" | "user" | "valve" | "custom" | "auto" (frontend-only)
  path: string;
}

export interface GameDto {
  app_id: number;
  name: string;
  source: string; // "steam" | "non-steam" | "heroic"
  executable: string | null;
  installed: boolean;
  /** Unix seconds, from localconfig.vdf. Null when never played (or non-Steam). */
  last_played: number | null;
  playtime_minutes: number | null;
  /** Heroic's per-game id; non-null only when source === "heroic". */
  heroic_id: string | null;
}

/** Result of a successful `inject_heroic` write, for the confirmation toast. */
export interface HeroicInjectResult {
  config_path: string;
  backup_path: string;
}

export interface StaleInfo {
  installed: string;
  catalog: string;
  updated: string;
}

/** One game's Proton log, for the diagnostics viewer. Mirrors ipc::ProtonLog.
 *  A missing file is `present: false` (not an error), so the viewer can prompt
 *  to enable logging rather than showing a failure. */
export interface ProtonLog {
  present: boolean;
  path: string;
  /** Tail of the log (last ~64 KB); empty when absent. */
  tail: string;
  size: number;
  /** True when the head was cut to fit the tail window. */
  truncated: boolean;
  /** Lines from the tail matching common error/warning markers. */
  error_lines: string[];
}

export interface UpdateInfo {
  available: boolean;
  current: string;
  latest: string;
  notes: string;
  html_url: string;
  download_url: string;
  sha256_url: string;
}

export interface Config {
  umu: boolean;
  runtime: string | null;
  env: Pair[];
  wrappers: Pair[];
  extra_env: string;
  umu_exe: string;
  umu_wineprefix: string;
  umu_gameid: string;
  game_args: string;
}

export interface Preset {
  name: string;
  game_appid: number | null;
  game_name: string | null;
  config: Config;
}

/** Mirrors store::Paths — user-supplied discovery paths, for systems the
 *  built-in candidates don't cover. */
export interface Paths {
  /** Extra Steam roots, tried *before* the built-in candidates. */
  steam_roots: string[];
  /** Extra library folders (each containing `steamapps/`). */
  steam_libraries: string[];
  /** Extra `compatibilitytools.d`-shaped dirs: one sub-folder per Proton
   *  build, each with a `compatibilitytool.vdf`. */
  proton_dirs: string[];
  /** Program overrides keyed by catalog `requires` name plus `umu-run`. A Rust
   *  `BTreeMap<String, String>`; a blank value means "use the bare name". */
  bins: Record<string, string>;
}

export interface Store {
  theme: string;
  presets: Preset[];
  game_memory: Record<string, Config>;
  dismissed_cachyos_build: string;
  dismissed_update_version: string;
  show_irrelevant: boolean;
  /** Show catalog entries tagged `tier = "advanced"`. Separate from
   *  `show_irrelevant`: that's about this machine, this is about this user. */
  show_advanced: boolean;
  hdr: boolean;
  /** Legacy "RDNA3/RDNA4" capability flag, superseded by `gpu_gen` as the UI
   *  control but still read as a fallback for pre-`gpu_gen` state files. */
  fsr4: boolean;
  /** AMD GPU generation. Drives three relevance capabilities: `fsr4` (either
   *  generation), `rdna3` and `rdna4` (generation-exclusive). A union rather
   *  than a bare string so a typo can't silently become "unset" — Rust keeps it
   *  a `String`, and `options_from_lists` treats anything unrecognised as "". */
  gpu_gen: GpuGen;
  protondb_auto: boolean;
  /** Appids pinned to the top of the library under every sort. A Rust
   *  `BTreeSet<u32>`, so it arrives sorted and must be sent back without
   *  duplicates. */
  favorites: number[];
  /** Last-used library sort: "recent" | "alpha" | "tuned" ("" before first use). */
  library_sort: string;
  last_session: Config | null;
  last_game_appid: number | null;
  /** UI density: "simple" | "advanced". A bare string (Rust keeps it a
   *  `String`); read through `app.uiMode`, which normalises the empty/unknown
   *  case to "simple". */
  ui_mode: string;
  paths: Paths;
  /** A reusable selection authored in Settings and applied to a game via a
   *  button. Null until the user saves one. Mirrors store::Store.global_profile
   *  (Rust `Option<Config>`). */
  global_profile: Config | null;
}

/// Mirrors explain::TokenKind (serde rename_all = "snake_case").
export type TokenKind =
  | "space"
  | "env"
  | "wrapper"
  | "wrapper_arg"
  | "separator"
  | "target"
  | "exe"
  | "game_arg"
  | "unknown";

/** One piece of a tokenized launch command. Concatenating every `text` in order
 *  reproduces the command byte-for-byte — never re-join these with spaces. */
export interface Token {
  text: string;
  kind: TokenKind;
  key: string | null;
}

/// Mirrors lint::Severity (serde rename_all = "lowercase").
export type Severity = "error" | "warning" | "info";

/** A one-click remedy for a notice. Applied entirely on the frontend with the
 *  existing toggleEnv / setEnvValue / toggleWrap helpers — there is no
 *  apply-fix command. */
export interface LintFix {
  label: string;
  /** Catalog keys (env or wrapper) to turn off. */
  disable: string[];
  /** Catalog env keys to turn on, with the value to set. */
  enable: Pair[];
}

export interface Notice {
  /** Stable rule id — usable as a list key. */
  id: string;
  severity: Severity;
  message: string;
  /** Catalog keys this notice implicates, for click-to-jump. */
  keys: string[];
  fix: LintFix | null;
}

/// Mirrors recipes::ChangeKind (serde rename_all = "snake_case").
export type ChangeKind = "enable" | "value_change" | "no_op" | "extra_env";

/** One key a recipe would touch. `from` is null when the key is currently off
 *  or lands in custom env. Mirrors recipes::RecipeChange. */
export interface RecipeChange {
  key: string;
  kind: ChangeKind;
  from: string | null;
  to: string;
  is_wrapper: boolean;
}

/// Mirrors diff::DiffStatus (serde rename_all = "kebab-case").
export type DiffStatus = "in-sync" | "drifted" | "not-applied" | "umu";

/**
 * Frontend-only. `DiffStatus` folded together with every case where the sync
 * pill must not appear at all: no game selected, a non-Steam shortcut (which has
 * no launch options to compare — an absent entry means *untracked*, never
 * "not applied"), umu mode, or a diff that hasn't been computed yet.
 */
export type SyncState = "in-sync" | "drifted" | "not-applied" | "hidden";

/** One key present on both sides with a different value. `key` is an env var
 *  name, a wrapper key, or the literal "game_args". */
export interface Change {
  key: string;
  current: string;
  built: string;
}

/** Semantic comparison of the built command against Steam's current launch
 *  options. Env and wrapper ordering, quoting and arg whitespace are
 *  deliberately normalised away — see diff.rs. */
export interface LaunchDiff {
  status: DiffStatus;
  /** Keys in the built command that Steam does not have. */
  added: string[];
  /** Keys Steam has that the built command does not. */
  removed: string[];
  changed: Change[];
  /** Tokens protongen cannot represent; any at all means drifted. */
  unmodeled: string[];
  game_args: Change | null;
}

export interface Tier {
  tier: string;
  total: number;
  confidence: string;
  trending: string;
  best: string;
}

/** A user config override that couldn't be parsed and was therefore ignored. */
/// Mirrors params::WarningKind (serde rename_all = "kebab-case").
export type WarningKind = "parse" | "path";

export interface ConfigWarning {
  kind: WarningKind;
  /** For "parse", the override file (`params.toml`). For "path", the Settings
   *  field label the path came from (`Steam root`, `Proton directory`, …). */
  file: string;
  path: string;
  error: string;
}

export interface Bootstrap {
  steam_root: string | null;
  load_error: string | null;
  catalog: Catalog;
  categories: string[];
  recipes: Recipe[];
  runtimes: RuntimeDto[];
  games: GameDto[];
  hardware: Hardware;
  store: Store;
  launch_options: Record<string, string>;
  compat_tools: Record<string, string>;
  requires_status: Record<string, boolean>;
  stale: StaleInfo | null;
  config_warnings: ConfigWarning[];
}

export function emptyConfig(): Config {
  return {
    umu: false,
    runtime: null,
    env: [],
    wrappers: [],
    extra_env: "",
    umu_exe: "",
    umu_wineprefix: "",
    umu_gameid: "",
    game_args: "",
  };
}
