/**
 * MangoHud overlay config: the `MANGOHUD_CONFIG` string <-> UI state round-trip.
 *
 * Pure string/struct functions with no Svelte dependency, extracted out of
 * `MangoHud.svelte` because they are the trickiest part of that component and
 * the round-trip property (`parse(build(c))` preserves everything the UI can
 * express) is only obvious when the two sit next to each other.
 */

export interface Metric {
  key: string;
  token: string;
  label: string;
}

export const METRICS: Metric[] = [
  { key: "fps", token: "fps", label: "FPS" },
  { key: "frametime", token: "frame_timing", label: "Frame timing" },
  { key: "cpu_load", token: "cpu_stats", label: "CPU load" },
  { key: "gpu_load", token: "gpu_stats", label: "GPU load" },
  { key: "cpu_temp", token: "cpu_temp", label: "CPU temp" },
  { key: "gpu_temp", token: "gpu_temp", label: "GPU temp" },
  { key: "ram", token: "ram", label: "RAM" },
  { key: "vram", token: "vram", label: "VRAM" },
  { key: "gpu_name", token: "gpu_name", label: "GPU name" },
];

export const POSITIONS: { value: string; label: string }[] = [
  { value: "", label: "Default (top-left)" },
  { value: "top-left", label: "Top left" },
  { value: "top-right", label: "Top right" },
  { value: "top-center", label: "Top center" },
  { value: "bottom-left", label: "Bottom left" },
  { value: "bottom-right", label: "Bottom right" },
  { value: "bottom-center", label: "Bottom center" },
];

export const COLOR_DEFS: { key: string; token: string; label: string; def: string }[] = [
  { key: "text", token: "text_color", label: "Text", def: "#ffffff" },
  { key: "gpu", token: "gpu_color", label: "GPU", def: "#2e9762" },
  { key: "cpu", token: "cpu_color", label: "CPU", def: "#2e97cb" },
  { key: "background", token: "background_color", label: "Background", def: "#000000" },
];

/** MangoHud's own defaults, used by the live preview when a colour is unset. */
export const DEFAULT_COLORS: Record<string, string> = Object.fromEntries(
  COLOR_DEFS.map((c) => [c.key, c.def]),
);

/** Everything the overlay builder can express, as plain data. */
export interface OverlayConfig {
  checks: Record<string, boolean>;
  fpsLimit: string;
  position: string;
  fontSize: string;
  roundCorners: string;
  horizontal: boolean;
  compact: boolean;
  bgAlphaOn: boolean;
  bgAlpha: string;
  alphaOn: boolean;
  alpha: string;
  colorOn: Record<string, boolean>;
  colorVal: Record<string, string>;
}

/**
 * Parse a `MANGOHUD_CONFIG` string back into UI state so a restored, applied or
 * undone overlay repopulates every control.
 *
 * Unknown tokens are dropped rather than preserved: the builder is a
 * *constructor* for the common case, not a lossless editor, and the raw string
 * always stays visible and editable on the MANGOHUD_CONFIG row itself.
 */
export function parseConfig(raw: string): OverlayConfig {
  const out: OverlayConfig = {
    checks: {},
    fpsLimit: "",
    position: "",
    fontSize: "",
    roundCorners: "",
    horizontal: false,
    compact: false,
    bgAlphaOn: false,
    bgAlpha: "0.4",
    alphaOn: false,
    alpha: "1",
    colorOn: {},
    colorVal: { ...DEFAULT_COLORS },
  };
  for (const t of raw
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)) {
    const m = METRICS.find((x) => x.token === t);
    if (m) {
      out.checks[m.key] = true;
      continue;
    }
    if (t === "horizontal") {
      out.horizontal = true;
      continue;
    }
    if (t === "hud_compact") {
      out.compact = true;
      continue;
    }
    const eq = t.indexOf("=");
    if (eq === -1) continue;
    const k = t.slice(0, eq);
    const v = t.slice(eq + 1);
    switch (k) {
      case "fps_limit":
        if (Number(v) > 0) out.fpsLimit = v;
        break;
      case "position":
        out.position = v;
        break;
      case "font_size":
        out.fontSize = v;
        break;
      case "round_corners":
        out.roundCorners = v;
        break;
      case "background_alpha":
        out.bgAlphaOn = true;
        out.bgAlpha = v;
        break;
      case "alpha":
        out.alphaOn = true;
        out.alpha = v;
        break;
      default: {
        const cd = COLOR_DEFS.find((c) => c.token === k);
        if (cd) {
          out.colorOn[cd.key] = true;
          out.colorVal[cd.key] = "#" + v.replace(/^#/, "");
        }
      }
    }
  }
  if (!Object.keys(out.checks).length) out.checks = { fps: true, frametime: true };
  return out;
}

/** Assemble the `MANGOHUD_CONFIG` string. Inverse of `parseConfig`. */
export function buildConfig(c: OverlayConfig): string {
  const parts: string[] = [];
  for (const m of METRICS) if (c.checks[m.key]) parts.push(m.token);
  if (c.position) parts.push(`position=${c.position}`);
  if (c.fontSize.trim()) parts.push(`font_size=${c.fontSize.trim()}`);
  if (c.roundCorners.trim()) parts.push(`round_corners=${c.roundCorners.trim()}`);
  if (c.horizontal) parts.push("horizontal");
  if (c.compact) parts.push("hud_compact");
  if (c.bgAlphaOn) parts.push(`background_alpha=${c.bgAlpha}`);
  if (c.alphaOn) parts.push(`alpha=${c.alpha}`);
  for (const cd of COLOR_DEFS)
    if (c.colorOn[cd.key]) parts.push(`${cd.token}=${c.colorVal[cd.key].replace(/^#/, "")}`);
  const n = parseInt(c.fpsLimit.trim(), 10);
  if (Number.isFinite(n) && n > 0) parts.push(`fps_limit=${n}`);
  return parts.join(",");
}

/** `#rgb`/`#rrggbb` -> `rgba(...)`, for the preview's background alpha. */
export function hexToRgba(hex: string, a: number): string {
  const h = hex.replace(/^#/, "");
  const n =
    h.length === 3
      ? h
          .split("")
          .map((c) => c + c)
          .join("")
      : h;
  const r = parseInt(n.slice(0, 2), 16) || 0;
  const g = parseInt(n.slice(2, 4), 16) || 0;
  const b = parseInt(n.slice(4, 6), 16) || 0;
  return `rgba(${r}, ${g}, ${b}, ${a})`;
}
