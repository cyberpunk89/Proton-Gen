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

export interface RuntimeDto {
  internal_name: string;
  display_name: string;
  kind: string; // "system" | "user" | "valve"
  path: string;
}

export interface GameDto {
  app_id: number;
  name: string;
  source: string; // "steam" | "non-steam"
  executable: string | null;
  installed: boolean;
  /** Unix seconds, from localconfig.vdf. Null when never played (or non-Steam). */
  last_played: number | null;
  playtime_minutes: number | null;
}

export interface StaleInfo {
  installed: string;
  catalog: string;
  updated: string;
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

export interface Store {
  theme: string;
  presets: Preset[];
  game_memory: Record<string, Config>;
  dismissed_cachyos_build: string;
  dismissed_update_version: string;
  show_irrelevant: boolean;
  hdr: boolean;
  fsr4: boolean;
  protondb_auto: boolean;
  last_session: Config | null;
  last_game_appid: number | null;
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
export interface ConfigWarning {
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
