import { invoke } from "@tauri-apps/api/core";
import type { Bootstrap, Config, Store, Tier } from "./types";
import { mockBootstrap, mockBuildCommand } from "./mock";

// True when running inside the Tauri webview (vs. a plain browser for design/dev).
const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const ipc = {
  bootstrap: () =>
    inTauri ? invoke<Bootstrap>("bootstrap") : Promise.resolve(mockBootstrap),

  buildCommand: (config: Config, protonPath: string | null) =>
    inTauri
      ? invoke<string>("build_command", { config, protonPath })
      : Promise.resolve(mockBuildCommand(config, protonPath)),

  parseCommand: (input: string) =>
    inTauri ? invoke<Config>("parse_command", { input }) : Promise.resolve(emptyParse()),

  applyRecipe: (index: number, config: Config) =>
    inTauri ? invoke<Config>("apply_recipe", { index, config }) : Promise.resolve(config),

  lint: (config: Config) =>
    inTauri ? invoke<string[]>("lint", { config }) : Promise.resolve([] as string[]),

  protondbUrl: (appid: number) =>
    inTauri
      ? invoke<string>("protondb_url", { appid })
      : Promise.resolve(`https://www.protondb.com/app/${appid}`),

  protondbFetch: (appid: number) =>
    inTauri
      ? invoke<Tier>("protondb_fetch", { appid })
      : Promise.resolve<Tier>({
          tier: "gold",
          total: 421,
          confidence: "strong",
          trending: "gold",
          best: "platinum",
        }),

  gameArt: (
    appId: number,
    source: string,
    kind: "portrait" | "hero" | "header",
    online: boolean,
  ) =>
    inTauri
      ? invoke<string | null>("game_art", { appId, source, kind, online })
      : Promise.resolve(null),

  saveStore: (store: Store) =>
    inTauri ? invoke<void>("save_store", { store }) : Promise.resolve(),
};

function emptyParse(): Config {
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
