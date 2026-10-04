/**
 * Lossless Scaling frame generation (lsfg-vk 2.x): the builder's state <->
 * `LSFGVK_*` env round-trip.
 *
 * lsfg-vk has four mutually exclusive ways to be set up for one launch, and the
 * builder models exactly those rather than the dozen raw variables:
 *
 * - `auto`    — nothing in the launch string; conf.toml's `active_in` matching
 *               decides (lsfg-vk's own default behaviour);
 * - `profile` — `LSFGVK_PROFILE=<name>`: one conf.toml profile, by name;
 * - `custom`  — `LSFGVK_ENV=1` + per-game settings: the launch string carries
 *               everything and conf.toml is not read at all;
 * - `off`     — `DISABLE_LSFGVK=1`.
 *
 * Pure functions, no Svelte, same shape as `optiscaler.ts`.
 */

import type { LsfgStatus } from "./types";

export type LsfgMode = "auto" | "profile" | "custom" | "off";

export interface LsfgSettings {
  mode: LsfgMode;
  profile: string;
  multiplier: number;
  /** 0.25 – 1.0 */
  flowScale: number;
  performance: boolean;
  /** LSFGVK_NO_FP16=1 */
  noFp16: boolean;
  /** LSFGVK_OVERRIDE_PRESENT_MODE=0 — let the game's own present mode through. */
  noVsyncOverride: boolean;
  /** LSFGVK_PRESERVE_SWAPCHAIN_IMAGE_COUNT=1 */
  preserveSwapchain: boolean;
}

/** Every variable the builder owns. Applying turns all of them off, then on
 *  only the ones the chosen mode needs — so switching mode never leaves a
 *  stale `LSFGVK_PROFILE` next to `LSFGVK_ENV=1` (which `lint.rs` flags).
 *  `LSFGVK_CONFIG` and the log variables are deliberately not owned: they are
 *  debugging knobs set by hand on their rows, and the builder leaves them be. */
export const LSFG_BUILDER_KEYS = [
  "LSFGVK_PROFILE",
  "LSFGVK_ENV",
  "DISABLE_LSFGVK",
  "LSFGVK_MULTIPLIER",
  "LSFGVK_FLOW_SCALE",
  "LSFGVK_PERFORMANCE_MODE",
  "LSFGVK_DLL_PATH",
  "LSFGVK_NO_FP16",
  "LSFGVK_OVERRIDE_PRESENT_MODE",
  "LSFGVK_PRESERVE_SWAPCHAIN_IMAGE_COUNT",
] as const;

export const MULTIPLIERS = [2, 3, 4, 5, 6] as const;

export function defaultLsfg(): LsfgSettings {
  return {
    mode: "auto",
    profile: "",
    multiplier: 2,
    flowScale: 1,
    performance: false,
    noFp16: false,
    noVsyncOverride: false,
    preserveSwapchain: false,
  };
}

/**
 * Read the builder state back from the launch env. `get` returns an enabled
 * row's value, or `null` when the row is off. The precedence mirrors lsfg-vk's:
 * DISABLE beats everything, env mode skips conf.toml (so beats a profile).
 */
export function readLsfg(get: (key: string) => string | null): LsfgSettings {
  const s = defaultLsfg();
  const on = (k: string) => {
    const v = get(k);
    return v != null && v !== "0";
  };

  const mult = Number(get("LSFGVK_MULTIPLIER"));
  if (Number.isInteger(mult) && mult > 1) s.multiplier = mult;
  const flow = Number(get("LSFGVK_FLOW_SCALE"));
  if (Number.isFinite(flow) && flow >= 0.25 && flow <= 1) s.flowScale = flow;
  s.performance = on("LSFGVK_PERFORMANCE_MODE");
  s.noFp16 = on("LSFGVK_NO_FP16");
  s.noVsyncOverride = get("LSFGVK_OVERRIDE_PRESENT_MODE") === "0";
  s.preserveSwapchain = on("LSFGVK_PRESERVE_SWAPCHAIN_IMAGE_COUNT");
  s.profile = get("LSFGVK_PROFILE") ?? "";

  if (on("DISABLE_LSFGVK")) s.mode = "off";
  else if (on("LSFGVK_ENV")) s.mode = "custom";
  else if (s.profile) s.mode = "profile";
  return s;
}

/** `1` -> "1.0", `0.5` -> "0.5", `0.85` -> "0.85". */
export function formatFlow(n: number): string {
  return n.toFixed(2).replace(/0$/, "");
}

/**
 * The `[key, value]` pairs to turn on for `s`; every other key in
 * [`LSFG_BUILDER_KEYS`] is turned off. `dllPath` is written only in custom
 * mode, and only when lsfg-vk couldn't find the DLL itself (conf.toml's
 * `dll` isn't read in env mode).
 */
export function buildLsfg(s: LsfgSettings, dllPath: string | null): [string, string][] {
  switch (s.mode) {
    case "auto":
      return [];
    case "off":
      return [["DISABLE_LSFGVK", "1"]];
    case "profile":
      return s.profile ? [["LSFGVK_PROFILE", s.profile]] : [];
    case "custom": {
      const out: [string, string][] = [
        ["LSFGVK_ENV", "1"],
        ["LSFGVK_MULTIPLIER", String(s.multiplier)],
        ["LSFGVK_FLOW_SCALE", formatFlow(s.flowScale)],
      ];
      if (s.performance) out.push(["LSFGVK_PERFORMANCE_MODE", "1"]);
      if (dllPath) out.push(["LSFGVK_DLL_PATH", dllPath]);
      if (s.noFp16) out.push(["LSFGVK_NO_FP16", "1"]);
      if (s.noVsyncOverride) out.push(["LSFGVK_OVERRIDE_PRESENT_MODE", "0"]);
      if (s.preserveSwapchain) out.push(["LSFGVK_PRESERVE_SWAPCHAIN_IMAGE_COUNT", "1"]);
      return out;
    }
  }
}

/**
 * The `LSFGVK_DLL_PATH` per-game mode needs, or `null`. Only when lsfg-vk
 * wouldn't find the DLL by itself: outside its default Steam roots, or known
 * only from conf.toml's `dll` — which env mode doesn't read.
 */
export function envDllPath(st: LsfgStatus | null): string | null {
  return st?.dll && (st.dll_needs_path || st.dll_source === "config") ? st.dll : null;
}

/** Whether a set of pairs from [`buildLsfg`] actually generates frames. */
export function lsfgGenerates(s: LsfgSettings): boolean {
  return s.mode === "custom" || (s.mode === "profile" && s.profile !== "");
}

/**
 * Does `pattern` (one `active_in` entry) match a game executable path? lsfg-vk
 * matches a bare binary / `.exe` name or a path suffix; this is the same test
 * against what protongen knows of the game, used only for a hint.
 */
export function activeInMatches(pattern: string, executable: string): boolean {
  const p = pattern.trim().replaceAll("\\", "/").toLowerCase();
  const exe = executable.trim().replaceAll("\\", "/").toLowerCase();
  if (!p || !exe) return false;
  const base = exe.slice(exe.lastIndexOf("/") + 1);
  return base === p || exe.endsWith(p.startsWith("/") ? p : `/${p}`);
}
