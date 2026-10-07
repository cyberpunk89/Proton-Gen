import { invoke } from "@tauri-apps/api/core";
import type {
  Bootstrap,
  Config,
  DiffStatus,
  HeroicInjectResult,
  LaunchDiff,
  LlmRequest,
  LlmSuggestion,
  LsfgStatus,
  MangohudExportResult,
  Notice,
  OptiscalerExtractResult,
  OptiscalerChannel,
  RuntimeUpdate,
  OptiscalerRelease,
  OptiscalerStatus,
  ParsedCommand,
  ProtonLog,
  TroubleshootRequest,
  TroubleshootResult,
  RecipeChange,
  Store,
  SteamUserConfig,
  Tier,
  Token,
  UpdateInfo,
  VkBasaltExportResult,
} from "./types";
import {
  mockBootstrap,
  mockBuildCommand,
  mockExplain,
  mockExportMangohudSystem,
  mockExportVkbasaltSystem,
  mockInjectHeroic,
  mockLaunchDiff,
  mockLaunchStatuses,
  mockLsfgStatus,
  mockNotices,
  mockPreviewRecipe,
  mockSteam,
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

  // Write the config's env vars + wrappers into a Heroic game's per-game config.
  // `appName` is the game's `heroic_id`.
  injectHeroic: (appName: string, config: Config) =>
    inTauri
      ? invoke<HeroicInjectResult>("inject_heroic", { appName, config })
      : Promise.resolve(mockInjectHeroic(appName, config)),

  /** Nexus's report on success; rejects with its error otherwise. */
  applyToNexus: (appId: number, code: string) =>
    inTauri
      ? invoke<string>("apply_to_nexus", { appId, code })
      : Promise.resolve(`Launch profile set (browser mock — nothing was written).`),

  // Best-effort: is Heroic currently running? It caches a game's settings in
  // memory at launch and can flush that stale copy back over whatever protongen
  // just wrote when it exits, so the confirm dialog checks this before writing
  // rather than let the user discover it after the fact. No real process tree
  // to inspect outside Tauri, so the mock always says "not running".
  heroicRunning: () => (inTauri ? invoke<boolean>("heroic_running") : Promise.resolve(false)),

  parseCommand: (input: string) =>
    inTauri
      ? invoke<ParsedCommand>("parse_command", { input })
      : Promise.resolve<ParsedCommand>({ config: emptyParse(), dropped: [] }),

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

  // Re-read only Steam's launch options + compat tools (no full scan), for the
  // window-focus refresh after the user pastes into Steam. `null` = no Steam.
  steamUserConfig: () =>
    inTauri
      ? invoke<SteamUserConfig | null>("steam_user_config")
      : Promise.resolve<SteamUserConfig>({
          launch_options: { ...mockSteam.launch_options },
          compat_tools: { ...mockSteam.compat_tools },
        }),

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

  // `appId` lets the backend check that game's folder for a manual OptiScaler
  // install stacked under the injected one; null for no game selected.
  lint: (config: Config, appId: number | null) =>
    inTauri ? invoke<Notice[]>("lint", { config, appId }) : Promise.resolve(mockNotices),

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

  // Read a game's Proton log (~/steam-<appid>.log) for the diagnostics viewer.
  // The mock has no filesystem, so it returns a small canned "present" log — just
  // enough to exercise the viewer and the AI coach (present + a couple of error
  // lines) under `pnpm dev`.
  readProtonLog: (appId: number) =>
    inTauri
      ? invoke<ProtonLog>("read_proton_log", { appId })
      : Promise.resolve<ProtonLog>({
          present: true,
          path: "~/steam-" + appId + ".log",
          tail:
            "info:  Game: eldenring.exe\n" +
            "info:  DXVK: v2.4\n" +
            "warn:  D3D11: unsupported feature level\n" +
            "err:   vulkan: device lost while presenting\n" +
            "info:  shader cache: 1423 entries\n",
          size: 4096,
          truncated: false,
          error_lines: [
            "warn:  D3D11: unsupported feature level",
            "err:   vulkan: device lost while presenting",
          ],
        }),

  // Analyze a game's Proton log with the configured local LLM. The mock returns
  // a canned suggestion (with one apply-able change) so the UI is exercisable in
  // browser-dev without a running server.
  llmAnalyze: (req: LlmRequest) =>
    inTauri
      ? invoke<LlmSuggestion>("llm_analyze", { req })
      : Promise.resolve<LlmSuggestion>({
          text: "**Mock analysis.** The log shows a 'device lost while presenting' error — a GPU hang, often driver or shader-cache related. As a first step, add the MangoHud overlay to watch frame times and confirm where the hitching starts.\n\n(Connect a local LLM in the real app for real suggestions.)",
          changes: [
            {
              key: "mangohud",
              value: "",
              kind: "wrap",
              reason: "Show frame times on-screen to pinpoint the stutter.",
            },
          ],
        }),

  // Diagnose a free-text symptom: recommend existing Fix recipes (by index) and
  // propose catalog changes. Mock recommends the "Game crashes at launch" fix
  // (index 3 in mock recipes) plus one change, to exercise both result types.
  llmTroubleshoot: (req: TroubleshootRequest) =>
    inTauri
      ? invoke<TroubleshootResult>("llm_troubleshoot", { req })
      : Promise.resolve<TroubleshootResult>({
          text:
            "**Mock diagnosis.** A crash right at launch is most often a shader-cache or runtime mismatch. The 'Game crashes at launch' recipe below applies the usual first-line fixes; if it persists, forcing the Wayland driver can help on some setups.\n\n(Connect a local LLM in the real app for real diagnoses.)",
          recipes: [3],
          changes: [
            {
              key: "PROTON_ENABLE_WAYLAND",
              value: "1",
              kind: "env",
              reason: "Use the native Wayland driver — sometimes avoids launch crashes.",
            },
          ],
        }),

  // List models the local LLM endpoint is serving (Settings picker / test).
  llmModels: () =>
    inTauri
      ? invoke<string[]>("llm_models")
      : Promise.resolve<string[]>(["gpt-oss-20b", "google/gemma-3-12b-qat"]),

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

  // Newer GE-Proton / proton-cachyos than installed. The mock reports one so
  // the banner can be seen under `pnpm dev`.
  checkRuntimeUpdates: () =>
    inTauri
      ? invoke<RuntimeUpdate[]>("check_runtime_updates")
      : Promise.resolve<RuntimeUpdate[]>([
          {
            family: "ge-proton",
            installed: "GE-Proton11-5",
            latest: "GE-Proton11-7",
            tag: "GE-Proton11-7",
            html_url: "https://github.com/GloriousEggroll/proton-ge-custom/releases/tag/GE-Proton11-7",
            installed_kind: "user",
          },
        ]),

  // No payload: the backend re-resolves the release itself rather than
  // trusting URLs from here.
  runUpdate: () => (inTauri ? invoke<void>("run_update") : Promise.resolve()),

  // lsfg-vk (Lossless Scaling frame generation): layer, conf.toml profiles and
  // Lossless.dll. Read-only and cheap; re-read whenever the builder opens.
  lsfgStatus: () =>
    inTauri ? invoke<LsfgStatus>("lsfg_status") : Promise.resolve(mockLsfgStatus()),

  // Whether `appId` already has an OptiScaler install to refresh. The mock has
  // no filesystem, so it reports "found" unconditionally — just enough to
  // exercise the fetch button under `pnpm dev`.
  optiscalerStatus: (appId: number) =>
    inTauri
      ? invoke<OptiscalerStatus>("optiscaler_status", { appId })
      : Promise.resolve<OptiscalerStatus>({
          install_dir: "/home/you/.local/share/Steam/steamapps/common/mock-game",
          found: true,
          proxies: ["dxgi.dll"],
          stray_dll: false,
        }),

  optiscalerLatest: (channel: OptiscalerChannel) =>
    inTauri
      ? invoke<OptiscalerRelease>("optiscaler_latest", { channel })
      : Promise.resolve<OptiscalerRelease>(
          channel === "nightly"
            ? {
                channel,
                repo: "optiscaler/OptiScaler-nightly",
                tag: "nightly-20260929",
                html_url: "https://github.com/optiscaler/OptiScaler-nightly/releases/tag/nightly-20260929",
                asset_name: "OptiScaler_v10.0.0-pre1_mock.7z",
              }
            : {
                channel,
                repo: "optiscaler/OptiScaler",
                tag: "v0.9.4",
                html_url: "https://github.com/optiscaler/OptiScaler/releases/tag/v0.9.4",
                asset_name: "Optiscaler_0.9.4-final.mock.7z",
              },
        ),

  optiscalerFetch: (appId: number, channel: OptiscalerChannel) =>
    inTauri
      ? invoke<OptiscalerExtractResult>("optiscaler_fetch", { appId, channel })
      : Promise.resolve<OptiscalerExtractResult>({
          tag: channel === "nightly" ? "nightly-20260929" : "v0.9.4",
          files_written: 12,
          ini_preserved: true,
          dll_name: "dxgi.dll",
        }),

  // Merge `config` into the real, system-wide ~/.config/MangoHud/MangoHud.conf.
  // The one command in this file gated to a single confirm-dialog call site
  // (see mangohud_export's doc comment for the read-only-by-contract exception).
  exportMangohudSystem: (config: string) =>
    inTauri
      ? invoke<MangohudExportResult>("export_mangohud_system", { config })
      : Promise.resolve(mockExportMangohudSystem(config)),

  // Read-only seed for the vkBasalt builder dialog — the real, current text
  // of ~/.config/vkBasalt/vkBasalt.conf, or "" if it doesn't exist yet.
  vkbasaltReadConfig: () =>
    inTauri ? invoke<string>("vkbasalt_read_config") : Promise.resolve(""),

  // Merge `config` into the real, system-wide ~/.config/vkBasalt/vkBasalt.conf.
  // Same read-only-by-contract exception as exportMangohudSystem above (see
  // vkbasalt_export's doc comment) — only ever called from its confirm dialog.
  exportVkbasaltSystem: (config: string) =>
    inTauri
      ? invoke<VkBasaltExportResult>("export_vkbasalt_system", { config })
      : Promise.resolve(mockExportVkbasaltSystem(config)),
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
