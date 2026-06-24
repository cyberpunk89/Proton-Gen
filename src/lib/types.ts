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
}

export interface StaleInfo {
  installed: string;
  catalog: string;
  updated: string;
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
  show_irrelevant: boolean;
  hdr: boolean;
  protondb_auto: boolean;
}

export interface Tier {
  tier: string;
  total: number;
  confidence: string;
  trending: string;
  best: string;
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
