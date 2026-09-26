// Share a preset as a copy-pasteable text code: `protongen:v1:` + base64 of its
// JSON. Clipboard-only by design — no file dialog, no new Tauri capability.
//
// A decoded code is untrusted text that ends up on a command line, so decode
// reads every field explicitly (never spreads the parsed object), bounds every
// size, and rebuilds the config on top of `emptyConfig()`.

import { emptyConfig } from "./types";
import type { Config } from "./types";

export const PRESET_CODE_PREFIX = "protongen:v1:";

const MAX_CODE = 32 * 1024;
const MAX_STR = 4096;
const MAX_ENV = 200;
const ENV_KEY = /^[A-Za-z_][A-Za-z0-9_]{0,127}$/;
const CONTROL = /[\u0000-\u001f\u007f]/;

export interface SharedPreset {
  name: string;
  config: Config;
}

export type DecodeResult =
  | { ok: true; preset: SharedPreset; droppedWrappers: string[] }
  | { ok: false; error: string };

export function isPresetCode(text: string): boolean {
  return text.trim().startsWith(PRESET_CODE_PREFIX);
}

export function encodePreset(p: SharedPreset): string {
  const json = JSON.stringify({ name: p.name, config: p.config });
  const bytes = new TextEncoder().encode(json);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return PRESET_CODE_PREFIX + btoa(bin);
}

/** `knownWrappers`: catalog wrapper keys; anything else is dropped and reported. */
export function decodePreset(code: string, knownWrappers: string[]): DecodeResult {
  const text = code.replace(/\s+/g, "");
  if (!text.startsWith(PRESET_CODE_PREFIX)) return fail("That isn't a protongen preset code.");
  if (text.length > MAX_CODE) return fail("That code is too large to be a preset.");

  let raw: unknown;
  try {
    const bin = atob(text.slice(PRESET_CODE_PREFIX.length));
    const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
    raw = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  } catch {
    return fail("That code is damaged or incomplete — copy it again.");
  }
  if (!isObject(raw) || !isObject(raw.config)) return fail("That code has no preset in it.");

  const name = typeof raw.name === "string" ? raw.name.trim() : "";
  if (!name || name.length > 64 || CONTROL.test(name)) return fail("The preset's name is invalid.");

  const c = raw.config;
  const config = emptyConfig();
  config.umu = c.umu === true;

  const env = pairs(c.env);
  if (env === null || env.length > MAX_ENV) return fail("The preset's environment variables are invalid.");
  if (env.some(([k]) => !ENV_KEY.test(k))) return fail("The preset names an invalid variable.");
  config.env = env;

  const wrappers = pairs(c.wrappers);
  if (wrappers === null) return fail("The preset's wrappers are invalid.");
  const droppedWrappers = wrappers.filter(([k]) => !knownWrappers.includes(k)).map(([k]) => k);
  config.wrappers = wrappers.filter(([k]) => knownWrappers.includes(k));

  for (const field of ["extra_env", "umu_exe", "umu_wineprefix", "umu_gameid", "game_args"] as const) {
    const v = c[field] ?? "";
    if (!isSafeString(v)) return fail("The preset contains an invalid value.");
    config[field] = v;
  }
  if (c.runtime != null) {
    if (!isSafeString(c.runtime)) return fail("The preset's runtime is invalid.");
    config.runtime = c.runtime;
  }

  return { ok: true, preset: { name, config }, droppedWrappers };
}

function fail(error: string): DecodeResult {
  return { ok: false, error };
}

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function isSafeString(v: unknown): v is string {
  return typeof v === "string" && v.length <= MAX_STR && !CONTROL.test(v);
}

/** `[string, string][]`, each string safe; null when anything else. */
function pairs(v: unknown): [string, string][] | null {
  if (v === undefined) return [];
  if (!Array.isArray(v)) return null;
  const out: [string, string][] = [];
  for (const p of v) {
    if (!Array.isArray(p) || p.length !== 2 || !isSafeString(p[0]) || !isSafeString(p[1])) return null;
    out.push([p[0], p[1]]);
  }
  return out;
}
