// Browser fallback data + handlers used when the app runs OUTSIDE Tauri (e.g.
// `vite` in a plain browser for design/dev). Under Tauri this module is unused.

import type { Bootstrap, Config, Notice, Token, TokenKind } from "./types";

export const mockBootstrap: Bootstrap = {
  steam_root: "/home/you/.local/share/Steam",
  load_error: null,
  catalog: {
    meta: { proton_cachyos_build: "20260601", updated: "2026-06-01" },
    wrappers: [
      {
        key: "gamescope",
        label: "gamescope",
        kind: "gamescope",
        default_value: "-W 2560 -H 1440 -f",
        requires: "gamescope",
        help: "Nested Wayland compositor for scaling, HDR and frame limiting.",
        details: "Runs the game inside a micro-compositor.",
        example: "gamescope -W 2560 -H 1440 -f -- %command%",
        url: "https://github.com/ValveSoftware/gamescope",
        gpu: null,
        needs: [],
      },
      {
        key: "gamemoderun",
        label: "gamemoderun",
        kind: "plain",
        default_value: "",
        requires: "gamemoderun",
        help: "Applies Feral GameMode performance tuning.",
        details: "CPU governor + IO priority tweaks while the game runs.",
        example: "gamemoderun %command%",
        url: "https://github.com/FeralInteractive/gamemode",
        gpu: null,
        needs: [],
      },
      {
        key: "mangohud",
        label: "mangohud",
        kind: "plain",
        default_value: "",
        requires: "mangohud",
        help: "Performance overlay (FPS, frametimes, temps).",
        details: "Configurable via MANGOHUD_CONFIG.",
        example: "mangohud %command%",
        url: "https://github.com/flightlessmango/MangoHud",
        gpu: null,
        needs: [],
      },
    ],
    envs: [
      {
        key: "PROTON_NO_NTSYNC",
        category: "Performance / Sync",
        default_value: "1",
        values: ["1", "0"],
        requires: null,
        help: "Disable the in-kernel ntsync driver (on by default; falls back to fsync/esync).",
        details: "ntsync is enabled by default in Proton 11; set this to disable it and fall back to fsync/esync.",
        example: "PROTON_NO_NTSYNC=1 %command%",
        url: null,
        gpu: null,
        needs: ["ntsync"],
      },
      {
        key: "DXVK_ASYNC",
        category: "Performance / Sync",
        default_value: "1",
        values: ["1"],
        requires: null,
        help: "Compile shaders asynchronously to reduce stutter.",
        details: null,
        example: null,
        url: null,
        gpu: null,
        needs: [],
      },
      {
        key: "DXVK_ENABLE_NVAPI",
        category: "NVIDIA",
        default_value: "1",
        values: ["1"],
        requires: null,
        help: "Expose NVAPI so DLSS / Reflex work.",
        details: null,
        example: null,
        url: null,
        gpu: "nvidia",
        needs: [],
      },
      {
        key: "PROTON_LOG",
        category: "Performance / Sync",
        default_value: "0",
        values: ["0", "1", "+all"],
        requires: null,
        help: "Proton log verbosity (3-option dropdown).",
        details: null,
        example: null,
        url: null,
        gpu: null,
        needs: [],
      },
      {
        key: "PROTON_ENABLE_WAYLAND",
        category: "Display / HDR",
        default_value: "1",
        values: ["1", "0"],
        requires: null,
        help: "Use the native Wayland driver.",
        details: null,
        example: null,
        url: null,
        gpu: null,
        needs: ["wayland"],
      },
      {
        key: "DXVK_HDR",
        category: "Display / HDR",
        default_value: "1",
        values: ["1", "0"],
        requires: null,
        help: "Enable HDR output through DXVK.",
        details: null,
        example: null,
        url: null,
        gpu: null,
        needs: ["hdr"],
      },
      {
        key: "MANGOHUD_CONFIG",
        category: "Logging / Debug",
        default_value: "fps_limit=0,cpu_temp,gpu_temp,ram,vram",
        values: [],
        requires: null,
        help: "MangoHud overlay config (comma-separated). Pairs with the MangoHud wrapper.",
        details: "Configures the MangoHud overlay display via comma-separated options.",
        example: "MANGOHUD_CONFIG=fps,cpu_temp mangohud %command%",
        url: "https://github.com/flightlessmango/MangoHud",
        gpu: null,
        needs: [],
      },
      {
        key: "MANGOHUD_CONFIGFILE",
        category: "Logging / Debug",
        default_value: "",
        values: [],
        requires: null,
        help: "Path to a MangoHud .conf file (used when MANGOHUD_CONFIG is unset).",
        details: "Points MangoHud at an on-disk config file instead of the inline string.",
        example: "MANGOHUD_CONFIGFILE=/home/user/.config/MangoHud/MangoHud.conf mangohud %command%",
        url: "https://github.com/flightlessmango/MangoHud",
        gpu: null,
        needs: [],
      },
    ],
  },
  categories: ["Performance / Sync", "NVIDIA", "Display / HDR"],
  recipes: [
    {
      name: "NVIDIA: DLSS + Reflex",
      kind: "profile",
      description: "Expose NVAPI to DXVK, auto-upgrade DLSS, enable Reflex low-latency.",
      symptom: null,
      gpu: "nvidia",
      needs: [],
      icon: "lightning",
      accent: "#76b900",
      tags: ["DLSS", "Reflex"],
      env: [["DXVK_ENABLE_NVAPI", "1"]],
      wrappers: [],
    },
    {
      name: "Low-latency competitive",
      kind: "profile",
      description: "GameMode + a MangoHud FPS readout (NTSync is on by default).",
      symptom: null,
      gpu: null,
      needs: ["ntsync"],
      icon: "gauge",
      accent: "#7aa2f7",
      tags: ["low-latency", "GameMode"],
      env: [],
      wrappers: [["gamemoderun", ""], ["mangohud", ""]],
    },
    {
      name: "HDR on Wayland",
      kind: "profile",
      description: "Native Wayland driver + DXVK HDR output (needs an HDR display).",
      symptom: null,
      gpu: null,
      needs: ["wayland", "hdr"],
      icon: "monitor",
      accent: "#f5bde6",
      tags: ["HDR", "Wayland"],
      env: [["PROTON_ENABLE_WAYLAND", "1"]],
      wrappers: [],
    },
    {
      name: "Game crashes at launch",
      kind: "fix",
      description: "Common crash workarounds plus a log to pinpoint the cause.",
      symptom: "Crashes before or at the main menu.",
      gpu: null,
      needs: [],
      icon: "wrench",
      accent: "#fab387",
      tags: ["logging"],
      env: [["DXVK_ASYNC", "1"]],
      wrappers: [],
    },
  ],
  runtimes: [
    {
      internal_name: "proton-cachyos-slr",
      display_name: "proton-cachyos-11.0-20260601 (steam linux runtime)",
      kind: "system",
      path: "/usr/share/steam/compatibilitytools.d/proton-cachyos-slr",
    },
    {
      internal_name: "GE-Proton11-1",
      display_name: "GE-Proton11-1",
      kind: "user",
      path: "/home/you/.local/share/Steam/compatibilitytools.d/GE-Proton11-1",
    },
  ],
  // Deliberately covers every branch of the metadata the library grid sorts on:
  // recently played, played a while back, uninstalled-but-previously-played, and
  // a non-Steam shortcut Steam keeps no per-app record for (all-null).
  games: [
    {
      app_id: 553850,
      name: "HELLDIVERS 2",
      source: "steam",
      executable: null,
      installed: true,
      last_played: 1785024000, // 2026-07-26
      playtime_minutes: 4210,
    },
    {
      app_id: 1245620,
      name: "ELDEN RING",
      source: "steam",
      executable: null,
      installed: true,
      last_played: 1783000000, // 2026-07-02
      playtime_minutes: 8640,
    },
    {
      app_id: 275850,
      name: "No Man's Sky",
      source: "steam",
      executable: null,
      installed: false,
      last_played: 1740000000, // 2025-02-19
      playtime_minutes: 312,
    },
    {
      app_id: 2001,
      name: "Heroic - Cyberpunk 2077",
      source: "non-steam",
      executable: "/games/cp2077/bin/x64/Cyberpunk2077.exe",
      installed: true,
      last_played: null,
      playtime_minutes: null,
    },
  ],
  hardware: {
    nvidia: true,
    amd: false,
    intel: false,
    wayland: true,
    kde: true,
    ntsync: true,
  },
  store: {
    theme: "mocha",
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
  },
  launch_options: { "553850": "PROTON_USE_NTSYNC=1 mangohud %command%" },
  compat_tools: { "553850": "proton-cachyos-slr" },
  requires_status: { gamescope: true, gamemoderun: true, mangohud: false },
  stale: null,
};

// Static stand-in for lint::warnings. Two real rule ids at two severities, one
// with a fix and one without, so the notices UI can be iterated under
// `pnpm dev` — the browser mock has no rule engine to derive them from.
export const mockNotices: Notice[] = [
  {
    id: "gplasync-anticheat",
    severity: "error",
    message:
      "PROTON_DXVK_GPLASYNC can trip kernel anti-cheat — avoid it in EAC/BattlEye games.",
    keys: ["PROTON_DXVK_GPLASYNC", "PROTON_EAC_RUNTIME"],
    fix: {
      label: "Disable PROTON_DXVK_GPLASYNC",
      disable: ["PROTON_DXVK_GPLASYNC"],
      enable: [],
    },
  },
  {
    id: "gamescope-vs-wayland",
    severity: "warning",
    message: "gamescope and PROTON_ENABLE_WAYLAND together can conflict — usually pick one.",
    keys: ["gamescope", "PROTON_ENABLE_WAYLAND"],
    fix: null,
  },
];

export function mockBuildCommand(config: Config, protonPath: string | null): string {
  const env = config.env.map(([k, v]) => `${k}=${v}`);
  const extra = config.extra_env.trim();
  if (extra) env.push(extra);
  const wraps = config.wrappers.map(([k, v]) =>
    k === "gamescope" ? `gamescope ${v} --`.trim() : k,
  );
  if (config.umu) {
    const lead = [`GAMEID=${config.umu_gameid || "umu-0"}`, `PROTONPATH=${protonPath ?? ""}`];
    return [...lead, ...env, ...wraps, "umu-run", config.umu_exe || "<game.exe>", config.game_args]
      .filter(Boolean)
      .join(" ");
  }
  return [...env, ...wraps, "%command%", config.game_args].filter(Boolean).join(" ");
}

// Regex stand-in for explain.rs. Good enough for browser dev; the real
// byte-exactness guarantee is the Rust round-trip test.
const WORD_OR_SPACE = /\s+|(?:"[^"]*"|'[^']*'|[^\s"'])+/g;

export function mockExplain(command: string): Token[] {
  const pieces = command.match(WORD_OR_SPACE) ?? [];
  const bare = (s: string) => s.replace(/["']/g, "");
  const isUmu = pieces.some((p) => bare(p) === "umu-run");
  const target = isUmu ? "umu-run" : "%command%";

  let pastTarget = false;
  let inGamescopeArgs = false;
  let postCount = 0;

  return pieces.map((text): Token => {
    if (/^\s+$/.test(text)) return { text, kind: "space", key: null };
    const w = bare(text);
    let kind: TokenKind;
    let key: string | null = null;

    if (pastTarget) {
      postCount += 1;
      kind = isUmu && postCount === 1 ? "exe" : "game_arg";
    } else if (w === target) {
      pastTarget = true;
      inGamescopeArgs = false;
      kind = "target";
    } else if (w === "--") {
      inGamescopeArgs = false;
      kind = "separator";
    } else if (inGamescopeArgs) {
      kind = "wrapper_arg";
    } else if (w === "gamescope") {
      inGamescopeArgs = true;
      kind = "wrapper";
      key = w;
    } else if (w === "gamemoderun" || w === "mangohud") {
      kind = "wrapper";
      key = w;
    } else if (/^[A-Za-z_]\w*=/.test(w)) {
      kind = "env";
      key = w.slice(0, w.indexOf("="));
    } else {
      kind = "unknown";
    }
    return { text, kind, key };
  });
}
