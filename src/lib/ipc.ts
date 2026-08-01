import { invoke } from "@tauri-apps/api/core";
import type {
  Bootstrap,
  Config,
  DiffStatus,
  LaunchDiff,
  Notice,
  RecipeChange,
  Store,
  Tier,
  Token,
  UpdateInfo,
} from "./types";
import {
  mockBootstrap,
  mockBuildCommand,
  mockExplain,
  mockLaunchDiff,
  mockLaunchStatuses,
  mockNotices,
  mockPreviewRecipe,
} from "./mock";

// True when running inside the Tauri webview (vs. a plain browser for design/dev).
// Exported because a few features exist only in the real shell (custom URL
// schemes, the native file dialog) and have to say so rather than no-op.
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const ipc = {
  bootstrap: () =>
    inTauri ? invoke<Bootstrap>("bootstrap") : Promise.resolve(mockBootstrap),

  // Re-scan the library (games, runtimes, shortcuts) without restarting the app.
  rescan: () =>
    inTauri ? invoke<Bootstrap>("rescan") : Promise.resolve(mockBootstrap),

  buildCommand: (config: Config, protonPath: string | null) =>
    inTauri
      ? invoke<string>("build_command", { config, protonPath })
      : Promise.resolve(mockBuildCommand(config, protonPath)),

  parseCommand: (input: string) =>
    inTauri ? invoke<Config>("parse_command", { input }) : Promise.resolve(emptyParse()),

  // Tokenize the preview for colouring/annotation. Tokens carry only a catalog
  // `key`; look help/details/url up in the already-loaded catalog.
  explainCommand: (command: string) =>
    inTauri
      ? invoke<Token[]>("explain_command", { command })
      : Promise.resolve(mockExplain(command)),

  // Semantic comparison of the built command against Steam's current launch
  // options. Both sides are re-parsed, so ordering/quoting differences don't
  // register as drift.
  launchDiff: (built: string, current: string) =>
    inTauri
      ? invoke<LaunchDiff>("launch_diff", { built, current })
      : Promise.resolve(mockLaunchDiff(built, current)),

  // Batch form of launchDiff for the library grid: one status per remembered
  // game. `launchOptions` is passed in rather than read from AppState, whose
  // discovery snapshot goes stale after a rescan.
  launchStatuses: (memory: Record<string, Config>, launchOptions: Record<string, string>) =>
    inTauri
      ? invoke<Record<string, DiffStatus>>("launch_statuses", { memory, launchOptions })
      : Promise.resolve(mockLaunchStatuses(memory, launchOptions)),

  applyRecipe: (index: number, config: Config) =>
    inTauri ? invoke<Config>("apply_recipe", { index, config }) : Promise.resolve(config),

  // What applying a recipe would change, without changing it.
  previewRecipe: (index: number, config: Config) =>
    inTauri
      ? invoke<RecipeChange[]>("preview_recipe", { index, config })
      : Promise.resolve(mockPreviewRecipe(index, config)),

  lint: (config: Config) =>
    inTauri ? invoke<Notice[]>("lint", { config }) : Promise.resolve(mockNotices),

  protondbUrl: (appid: number) =>
    inTauri
      ? invoke<string>("protondb_url", { appid })
      : Promise.resolve(`https://www.protondb.com/app/${appid}`),

  protondbFetch: (appid: number) =>
    inTauri
      ? invoke<Tier>("protondb_fetch", { appid })
      : // Deliberately a game that has regressed: trending below the overall
        // tier and a better best-reported, so the dev path exercises both of
        // the chip's secondary readouts.
        Promise.resolve<Tier>({
          tier: "gold",
          total: 421,
          confidence: "strong",
          trending: "silver",
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

  checkForUpdate: () =>
    inTauri
      ? invoke<UpdateInfo>("check_for_update")
      : Promise.resolve<UpdateInfo>({
          available: false,
          current: "0.0.0",
          latest: "0.0.0",
          notes: "",
          html_url: "",
          download_url: "",
          sha256_url: "",
        }),

  runUpdate: (info: UpdateInfo) =>
    inTauri ? invoke<void>("run_update", { info }) : Promise.resolve(),
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
