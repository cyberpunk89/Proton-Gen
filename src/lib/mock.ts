// Browser fallback data + handlers used when the app runs OUTSIDE Tauri (e.g.
// `vite` in a plain browser for design/dev). Under Tauri this module is unused.

import type {
  Bootstrap,
  Change,
  Config,
  DiffStatus,
  HeroicInjectResult,
  LaunchDiff,
  Notice,
  RecipeChange,
  Token,
  TokenKind,
} from "./types";

/** A blank Config, for seeding mock per-game memory. Local rather than
 *  `types.emptyConfig()` to keep mock.ts free of runtime imports. */
function emptyMockConfig(): Config {
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
      heroic_id: null,
    },
    {
      app_id: 1245620,
      name: "ELDEN RING",
      source: "steam",
      executable: null,
      installed: true,
      last_played: 1783000000, // 2026-07-02
      playtime_minutes: 8640,
      heroic_id: null,
    },
    {
      app_id: 275850,
      name: "No Man's Sky",
      source: "steam",
      executable: null,
      installed: false,
      last_played: 1740000000, // 2025-02-19
      playtime_minutes: 312,
      heroic_id: null,
    },
    {
      app_id: 0x80000001,
      name: "Crimson Desert (Heroic)",
      source: "heroic",
      executable: "/home/u/Games/FitGirl/crimson-desert/pfx/drive_c/Crimson Desert/bin64/CrimsonDesert.exe",
      installed: true,
      last_played: null,
      playtime_minutes: null,
      heroic_id: "7Hm5qmyaYmaSZ45Mqo3u4s",
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
    // Seeded so the library grid's per-game sync badges have something to show
    // under `pnpm dev`: 1245620 matches its launch options exactly (in-sync),
    // 553850 does not (drifted), 275850 has none set at all (not-applied).
    //
    // 275850 also carries PROTON_ENABLE_NVAPI, which is deliberately *not* in
    // the mock catalog (a552034 renamed it to PROTON_DISABLE_NVAPI upstream).
    // That makes the #62 recovery path — a stale key re-homed into the
    // custom-env field instead of silently vanishing — reachable in the browser
    // dev path just by selecting the game.
    game_memory: {
      "1245620": emptyMockConfig(),
      "553850": { ...emptyMockConfig(), wrappers: [["mangohud", ""]] },
      "275850": {
        ...emptyMockConfig(),
        env: [
          ["DXVK_ASYNC", "1"],
          ["PROTON_ENABLE_NVAPI", "1"],
        ],
      },
    },
    dismissed_cachyos_build: "",
    dismissed_update_version: "",
    show_irrelevant: false,
    hdr: false,
    fsr4: false,
    protondb_auto: false,
    // One favourite so the pin-to-top behaviour is visible under `pnpm dev`
    // without having to click a star first.
    favorites: [275850],
    library_sort: "",
    last_session: null,
    last_game_appid: null,
    // One seeded entry so the Paths section isn't empty under `pnpm dev`.
    paths: {
      steam_roots: [],
      steam_libraries: [],
      proton_dirs: ["/opt/proton-builds"],
      bins: {},
    },
  },
  // 553850 drifts against anything the builder produces; 1245620 is exactly
  // what `mockBuildCommand` emits for a freshly-reset config, so opening it
  // lands on the **in-sync** state with no clicks. Without that second entry
  // in-sync is unreachable under `pnpm dev` and the UI can't be iterated. The
  // bare "%command%" looks odd for launch options, and that is the point: it
  // is the default build output, verbatim.
  launch_options: {
    "553850": "PROTON_USE_NTSYNC=1 mangohud %command%",
    "1245620": "%command%",
  },
  compat_tools: { "553850": "proton-cachyos-slr" },
  // `umu-run` is seeded from Bins rather than a catalog `requires` entry — it is
  // neither a wrapper nor an env var, but an entire mode depends on it.
  requires_status: { gamescope: true, gamemoderun: true, mangohud: false, "umu-run": true },
  stale: null,
  config_warnings: [],
};

// Static stand-in for lint::warnings. Three real rule ids across three
// severities, two with a fix and one without, so the notices UI can be iterated
// under `pnpm dev` — the browser mock has no rule engine to derive them from.
//
// `hdr-needs-presentation` earns its place beyond severity coverage: DXVK_HDR is
// gated behind the opt-in `hdr` capability, so under the mock hardware it is
// filtered out of the panel. That makes it the one notice here whose jump link
// exercises revealParam's relevance guard — the case where a jump would
// otherwise land on a row that isn't rendered.
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
  {
    id: "hdr-needs-presentation",
    severity: "info",
    message: "HDR needs PROTON_ENABLE_WAYLAND=1 or gamescope with --hdr-enabled to take effect.",
    keys: ["DXVK_HDR"],
    fix: {
      label: "Enable PROTON_ENABLE_WAYLAND=1",
      disable: [],
      enable: [["PROTON_ENABLE_WAYLAND", "1"]],
    },
  },
];

/**
 * Stand-in for `recipes::diff`. Unlike `applyRecipe` (a no-op stub, since the
 * merge itself lives in Rust), this one computes a real answer from the mock
 * catalog — the preview chip is pure presentation of this data, so a stub would
 * make it unreviewable under `pnpm dev`.
 *
 * Mirrors the Rust classification: off → enable, on with a different value →
 * value_change, on with the same value → no_op, absent from the catalog →
 * extra_env (or no_op when already present in the custom-env string).
 */
export function mockPreviewRecipe(index: number, config: Config): RecipeChange[] {
  const recipe = mockBootstrap.recipes[index];
  if (!recipe) return [];

  const envOn = new Map(config.env);
  const wrapOn = new Map(config.wrappers);
  const extra = config.extra_env.split(/\s+/).filter(Boolean);
  const out: RecipeChange[] = [];

  for (const [key, value] of recipe.wrappers) {
    if (!mockBootstrap.catalog.wrappers.some((w) => w.key === key)) continue;
    const from = wrapOn.get(key);
    out.push({
      key,
      kind: from === undefined ? "enable" : value && from !== value ? "value_change" : "no_op",
      from: from ?? null,
      to: value || (from ?? ""),
      is_wrapper: true,
    });
  }

  for (const [key, value] of recipe.env) {
    if (!mockBootstrap.catalog.envs.some((e) => e.key === key)) {
      out.push({
        key,
        kind: extra.includes(`${key}=${value}`) ? "no_op" : "extra_env",
        from: null,
        to: value,
        is_wrapper: false,
      });
      continue;
    }
    const from = envOn.get(key);
    out.push({
      key,
      kind: from === undefined ? "enable" : from !== value ? "value_change" : "no_op",
      from: from ?? null,
      to: value,
      is_wrapper: false,
    });
  }

  return out;
}

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

export function mockInjectHeroic(appName: string, _config: Config): HeroicInjectResult {
  const dir = `~/.config/heroic/GamesConfig/${appName}`;
  return { config_path: `${dir}.json`, backup_path: `${dir}.json.protongen-1755290000.bak` };
}

// Reduced stand-in for diff.rs::compare. Understands the same normalisations
// (env order, quoting, wrapper order, arg whitespace) over the mock's small
// token vocabulary; the real guarantees live in the Rust tests.
const MOCK_WRAPPERS = ["gamemoderun", "mangohud"];

function mockNormalForm(command: string): {
  map: Record<string, string>;
  unmodeled: string[];
  gameArgs: string;
} {
  const bare = (s: string) => s.replace(/["']/g, "");
  const toks = (command.match(/(?:"[^"]*"|'[^']*'|[^\s"'])+/g) ?? []).map(bare);
  const isUmu = toks.includes("umu-run");
  const at = toks.indexOf(isUmu ? "umu-run" : "%command%");
  const pre = at === -1 ? toks : toks.slice(0, at);
  const post = at === -1 ? [] : toks.slice(at + 1);

  const map: Record<string, string> = {};
  const unmodeled: string[] = [];
  for (let i = 0; i < pre.length; i++) {
    const t = pre[i];
    if (t === "gamescope") {
      const args: string[] = [];
      while (++i < pre.length && pre[i] !== "--") args.push(pre[i]);
      map.gamescope = args.join(" ");
    } else if (MOCK_WRAPPERS.includes(t)) {
      map[t] = "";
    } else if (/^[A-Za-z_]\w*=/.test(t)) {
      map[t.slice(0, t.indexOf("="))] = t.slice(t.indexOf("=") + 1);
    } else {
      unmodeled.push(t);
    }
  }
  // In umu mode the first post-target token is the exe, not a game arg.
  return { map, unmodeled, gameArgs: (isUmu ? post.slice(1) : post).join(" ") };
}

export function mockLaunchDiff(built: string, current: string): LaunchDiff {
  const b = mockNormalForm(built);
  const c = mockNormalForm(current);
  const added = Object.keys(b.map).filter((k) => !(k in c.map)).sort();
  const removed = Object.keys(c.map).filter((k) => !(k in b.map)).sort();
  const changed: Change[] = Object.keys(b.map)
    .filter((k) => k in c.map && c.map[k] !== b.map[k])
    .sort()
    .map((k) => ({ key: k, current: c.map[k], built: b.map[k] }));
  const unmodeled = [...new Set([...c.unmodeled, ...b.unmodeled])].sort();
  const game_args =
    b.gameArgs === c.gameArgs
      ? null
      : { key: "game_args", current: c.gameArgs, built: b.gameArgs };

  const identical =
    !added.length && !removed.length && !changed.length && !unmodeled.length && !game_args;
  const status = built.includes("umu-run")
    ? "umu"
    : !current.trim()
      ? "not-applied"
      : identical
        ? "in-sync"
        : "drifted";

  return { status, added, removed, changed, unmodeled, game_args };
}

export function mockLaunchStatuses(
  memory: Record<string, Config>,
  launchOptions: Record<string, string>,
): Record<string, DiffStatus> {
  const out: Record<string, DiffStatus> = {};
  for (const [appid, config] of Object.entries(memory)) {
    out[appid] = config.umu
      ? "umu"
      : mockLaunchDiff(mockBuildCommand(config, null), launchOptions[appid] ?? "").status;
  }
  return out;
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
