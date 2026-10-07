/**
 * gamescope's arguments <-> builder state, for the gamescope builder.
 *
 * The wrapper's value is the argument string between `gamescope` and `--`,
 * emitted verbatim (it *is* shell — see `builder::sh_quote`'s note). This
 * models the flags people actually reach for and keeps everything else in
 * `passthrough`, in order, so opening the builder on a hand-written line and
 * pressing Apply never drops a flag it doesn't know. Same shape as
 * `mangohud.ts` / `vkbasalt.ts`: pure functions, and a round trip
 * (`parse(build(c))` keeps everything the UI expresses).
 *
 * Flag names are from `gamescope --help` (3.16). Short and long spellings
 * both parse; build always emits the short form where one exists.
 */

import { tokenizeEnv, shQuote } from "./shell";

export const FILTERS = [
  { value: "", label: "Default (linear)" },
  { value: "fsr", label: "AMD FSR 1" },
  { value: "nis", label: "NVIDIA NIS" },
  { value: "pixel", label: "Pixel art" },
  { value: "nearest", label: "Nearest" },
  { value: "linear", label: "Linear" },
];

export const SCALERS = [
  { value: "", label: "Default (auto)" },
  { value: "auto", label: "Auto" },
  { value: "integer", label: "Integer" },
  { value: "fit", label: "Fit" },
  { value: "fill", label: "Fill" },
  { value: "stretch", label: "Stretch" },
];

/** Common game (nested) resolutions, offered as one-click presets. */
export const GAME_RESOLUTIONS: { label: string; w: number; h: number }[] = [
  { label: "720p", w: 1280, h: 720 },
  { label: "900p", w: 1600, h: 900 },
  { label: "1080p", w: 1920, h: 1080 },
  { label: "1440p", w: 2560, h: 1440 },
  { label: "4K", w: 3840, h: 2160 },
];

export interface GamescopeConfig {
  /** Output (window/screen) size, -W / -H. "" = gamescope's default. */
  outW: string;
  outH: string;
  /** Game (nested) size, -w / -h — render below output and upscale. */
  gameW: string;
  gameH: string;
  /** Nested refresh, -r. */
  refresh: string;
  /** Upscale filter, -F. */
  filter: string;
  /** FSR/NIS sharpness 0 (sharpest) – 20, --sharpness. */
  sharpness: string;
  /** -S / --scaler. */
  scaler: string;
  fullscreen: boolean;
  borderless: boolean;
  hdr: boolean;
  adaptiveSync: boolean;
  grabCursor: boolean;
  /** --mangoapp: MangoHud drawn by gamescope (use instead of the mangohud wrapper). */
  mangoapp: boolean;
  /** -e / --steam: Steam integration (needed for the Steam overlay inside). */
  steam: boolean;
  /** --framerate-limit. */
  fpsLimit: string;
  /** Arguments this builder doesn't model, in their original order. */
  passthrough: string[];
}

export function emptyGamescope(): GamescopeConfig {
  return {
    outW: "",
    outH: "",
    gameW: "",
    gameH: "",
    refresh: "",
    filter: "",
    sharpness: "",
    scaler: "",
    fullscreen: false,
    borderless: false,
    hdr: false,
    adaptiveSync: false,
    grabCursor: false,
    mangoapp: false,
    steam: false,
    fpsLimit: "",
    passthrough: [],
  };
}

type ValueKey = "outW" | "outH" | "gameW" | "gameH" | "refresh" | "filter" | "sharpness" | "scaler" | "fpsLimit";
type FlagKey = "fullscreen" | "borderless" | "hdr" | "adaptiveSync" | "grabCursor" | "mangoapp" | "steam";

/** Flags that take a value, by every spelling. */
const VALUE_FLAGS: Record<string, ValueKey> = {
  "-W": "outW",
  "--output-width": "outW",
  "-H": "outH",
  "--output-height": "outH",
  "-w": "gameW",
  "--nested-width": "gameW",
  "-h": "gameH",
  "--nested-height": "gameH",
  "-r": "refresh",
  "--nested-refresh": "refresh",
  "-F": "filter",
  "--filter": "filter",
  "--sharpness": "sharpness",
  "--fsr-sharpness": "sharpness",
  "-S": "scaler",
  "--scaler": "scaler",
  "--framerate-limit": "fpsLimit",
};

const BOOL_FLAGS: Record<string, FlagKey> = {
  "-f": "fullscreen",
  "--fullscreen": "fullscreen",
  "-b": "borderless",
  "--borderless": "borderless",
  "--hdr-enabled": "hdr",
  "--adaptive-sync": "adaptiveSync",
  "--force-grab-cursor": "grabCursor",
  "--mangoapp": "mangoapp",
  "-e": "steam",
  "--steam": "steam",
};

/** Parse a gamescope argument string (the wrapper's value). */
export function parseGamescope(args: string): GamescopeConfig {
  const c = emptyGamescope();
  const words = tokenizeEnv(args);
  for (let i = 0; i < words.length; i++) {
    let w = words[i];
    // `--flag=value` spelling.
    let inline: string | null = null;
    const eq = w.startsWith("--") ? w.indexOf("=") : -1;
    if (eq > 0) {
      inline = w.slice(eq + 1);
      w = w.slice(0, eq);
    }
    if (w in VALUE_FLAGS) {
      const v = inline ?? words[i + 1];
      if (v !== undefined && (inline !== null || !v.startsWith("-"))) {
        c[VALUE_FLAGS[w]] = v;
        if (inline === null) i++;
        continue;
      }
    } else if (w in BOOL_FLAGS && inline === null) {
      c[BOOL_FLAGS[w]] = true;
      continue;
    }
    // Older single-letter upscalers: -U = FSR, -Y = NIS.
    if (w === "-U" && inline === null) {
      c.filter = "fsr";
      continue;
    }
    if (w === "-Y" && inline === null) {
      c.filter = "nis";
      continue;
    }
    c.passthrough.push(words[i]);
  }
  return c;
}

/** Build the argument string. Inverse of `parseGamescope`. */
export function buildGamescope(c: GamescopeConfig): string {
  const out: string[] = [];
  const val = (flag: string, v: string) => {
    const t = v.trim();
    if (t) out.push(flag, t);
  };
  val("-W", c.outW);
  val("-H", c.outH);
  val("-w", c.gameW);
  val("-h", c.gameH);
  val("-r", c.refresh);
  val("-F", c.filter);
  if (c.filter === "fsr" || c.filter === "nis") val("--sharpness", c.sharpness);
  val("-S", c.scaler);
  val("--framerate-limit", c.fpsLimit);
  if (c.fullscreen) out.push("-f");
  if (c.borderless) out.push("-b");
  if (c.hdr) out.push("--hdr-enabled");
  if (c.adaptiveSync) out.push("--adaptive-sync");
  if (c.grabCursor) out.push("--force-grab-cursor");
  if (c.mangoapp) out.push("--mangoapp");
  if (c.steam) out.push("-e");
  // Passthrough words were unquoted by the tokenizer; re-quote any that need it.
  out.push(...c.passthrough.map(shQuote));
  return out.join(" ");
}

/** Problems worth a warning line under the builder — not errors, it still builds. */
export function gamescopeWarnings(c: GamescopeConfig, opts: { mangohudWrapper: boolean }): string[] {
  const w: string[] = [];
  const num = (s: string) => (s.trim() ? Number(s) : null);
  for (const [label, v] of [
    ["Output width", c.outW],
    ["Output height", c.outH],
    ["Game width", c.gameW],
    ["Game height", c.gameH],
    ["Refresh", c.refresh],
    ["Frame-rate limit", c.fpsLimit],
  ] as const) {
    const n = num(v);
    if (n !== null && (!Number.isInteger(n) || n <= 0)) w.push(`${label} should be a positive whole number.`);
  }
  if ((c.gameW.trim() === "") !== (c.gameH.trim() === "")) w.push("Set both game width and height, or neither.");
  if ((c.outW.trim() === "") !== (c.outH.trim() === "")) w.push("Set both output width and height, or neither.");
  const gw = num(c.gameW);
  const ow = num(c.outW);
  if (c.filter && gw !== null && ow !== null && gw >= ow) {
    w.push("The upscale filter only does something when the game renders smaller than the output.");
  }
  if (c.mangoapp && opts.mangohudWrapper) {
    w.push("--mangoapp and the mangohud wrapper both draw an overlay — keep one.");
  }
  const s = num(c.sharpness);
  if (s !== null && (s < 0 || s > 20)) w.push("Sharpness runs from 0 (sharpest) to 20.");
  return w;
}
