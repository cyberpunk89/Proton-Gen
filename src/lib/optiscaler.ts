/**
 * OptiScaler config: the `PROTON_OPTISCALER_CONFIG` string <-> UI state
 * round-trip.
 *
 * proton-cachyos passes OptiScaler.ini settings inline as a semicolon-separated
 * list of `{Section}.{Key}={Value}` entries (see PROTON_OPTISCALER_CONFIG in
 * params.toml). That is the same shape as MANGOHUD_CONFIG, so this mirrors
 * `mangohud.ts`: pure string/struct functions with no Svelte dependency, with a
 * round-trip property (`parse(build(c))` preserves everything the UI expresses)
 * that is only obvious when the two sit next to each other.
 *
 * Keys and accepted values are from the OptiScaler.ini reference
 * (github.com/optiscaler/OptiScaler). Only the widely-used knobs are surfaced;
 * anything else is still reachable by hand-editing the config row.
 */

export interface Choice {
  value: string;
  label: string;
}

/** A "leave at OptiScaler's default" option — emitted as nothing. */
const DEFAULT: Choice = { value: "", label: "Default (auto)" };

/** [Upscalers] Dx12Upscaler — the one most games use. */
export const DX12_UPSCALERS: Choice[] = [
  DEFAULT,
  { value: "dlss", label: "DLSS" },
  { value: "ffx", label: "FSR (2.3 / 3.1 / 4.x)" },
  { value: "fsr21", label: "FSR 2.1" },
  { value: "fsr22", label: "FSR 2.2" },
  { value: "xess", label: "XeSS" },
];

/** [Upscalers] Dx11Upscaler. */
export const DX11_UPSCALERS: Choice[] = [
  DEFAULT,
  { value: "fsr22", label: "FSR 2.2 (native DX11)" },
  { value: "fsr31", label: "FSR 3.1 (native DX11)" },
  { value: "xess", label: "XeSS (Arc only)" },
];

/** [Upscalers] VulkanUpscaler. */
export const VULKAN_UPSCALERS: Choice[] = [
  DEFAULT,
  { value: "fsr21", label: "FSR 2.1 (native VK)" },
  { value: "fsr22", label: "FSR 2.2 (native VK)" },
  { value: "ffx", label: "FSR 2.3 / 3.1 (native VK)" },
];

/** [FrameGen] FGInput — where generated frames come from. */
export const FG_INPUTS: Choice[] = [
  { value: "nofg", label: "None" },
  { value: "dlssg", label: "DLSS-G" },
  { value: "nvngxfg", label: "NVNGX FG" },
  { value: "fsrfg", label: "FSR FG" },
  { value: "fsrfg30", label: "FSR FG 3.0" },
  { value: "upscaler", label: "Upscaler" },
];

/** [FrameGen] FGOutput — the frame-gen backend that presents them. */
export const FG_OUTPUTS: Choice[] = [
  { value: "nofg", label: "None" },
  { value: "fsrfg", label: "FSR FG" },
  { value: "xefg", label: "XeSS FG" },
  { value: "dlssg", label: "DLSS-G" },
];

/** [Sharpness] Shader — the sharpening filter. */
export const SHARPEN_SHADERS: Choice[] = [
  { value: "rcas", label: "RCAS" },
  { value: "da", label: "DA" },
  { value: "lcda", label: "LCDA" },
];

/** Everything the OptiScaler builder can express, as plain data. */
export interface OptiScalerConfig {
  dx12Upscaler: string;
  dx11Upscaler: string;
  vulkanUpscaler: string;
  outputScalingOn: boolean;
  outputScalingMult: string; // "0.5" – "3.0"
  sharpenOn: boolean;
  sharpenShader: string; // rcas | da | lcda
  sharpness: string; // "0.0" – "1.0"
  frameGenOn: boolean;
  fgInput: string;
  fgOutput: string;
  dlssPresetOn: boolean;
  dlssPreset: string; // "0" – "15"
}

export function emptyOptiScaler(): OptiScalerConfig {
  return {
    dx12Upscaler: "",
    dx11Upscaler: "",
    vulkanUpscaler: "",
    outputScalingOn: false,
    outputScalingMult: "1.5",
    sharpenOn: false,
    sharpenShader: "rcas",
    sharpness: "0.3",
    frameGenOn: false,
    fgInput: "dlssg",
    fgOutput: "fsrfg",
    dlssPresetOn: false,
    dlssPreset: "0",
  };
}

/**
 * Parse a `PROTON_OPTISCALER_CONFIG` string into the builder's state.
 *
 * Tolerant on purpose: unknown `Section.Key` entries are ignored (they round to
 * nothing on the way back out — a hand-written config with exotic keys is
 * better edited on the raw row), section/key matching is case-insensitive, and
 * whitespace around tokens is trimmed.
 */
export function parseOptiScaler(str: string): OptiScalerConfig {
  const c = emptyOptiScaler();
  const map = new Map<string, string>();
  for (const entry of str.split(";")) {
    const eq = entry.indexOf("=");
    if (eq < 0) continue;
    const key = entry.slice(0, eq).trim().toLowerCase();
    const val = entry.slice(eq + 1).trim();
    if (key) map.set(key, val);
  }
  const get = (k: string) => map.get(k.toLowerCase());
  const bool = (v: string | undefined) => v?.toLowerCase() === "true";

  if (get("upscalers.dx12upscaler")) c.dx12Upscaler = get("upscalers.dx12upscaler")!;
  if (get("upscalers.dx11upscaler")) c.dx11Upscaler = get("upscalers.dx11upscaler")!;
  if (get("upscalers.vulkanupscaler")) c.vulkanUpscaler = get("upscalers.vulkanupscaler")!;

  if (map.has("outputscaling.enabled")) {
    c.outputScalingOn = bool(get("outputscaling.enabled"));
    if (get("outputscaling.multiplier")) c.outputScalingMult = get("outputscaling.multiplier")!;
  }

  if (map.has("sharpness.overridesharpness")) {
    c.sharpenOn = bool(get("sharpness.overridesharpness"));
    if (get("sharpness.shader")) c.sharpenShader = get("sharpness.shader")!;
    if (get("sharpness.sharpness")) c.sharpness = get("sharpness.sharpness")!;
  }

  if (map.has("framegen.enabled")) {
    c.frameGenOn = bool(get("framegen.enabled"));
    if (get("framegen.fginput")) c.fgInput = get("framegen.fginput")!;
    if (get("framegen.fgoutput")) c.fgOutput = get("framegen.fgoutput")!;
  }

  if (map.has("dlss.renderpresetoverride")) {
    c.dlssPresetOn = bool(get("dlss.renderpresetoverride"));
    if (get("dlss.renderpresetforall")) c.dlssPreset = get("dlss.renderpresetforall")!;
  }

  return c;
}

/**
 * Build the `PROTON_OPTISCALER_CONFIG` string from the builder's state.
 *
 * Only set options are emitted, so a config that changes nothing is the empty
 * string. Section sub-values (multiplier, sharpness amount, FG in/out) are
 * emitted only when their section is enabled, so `parse(build(c))` equals `c`
 * for everything the UI can express.
 */
export function buildOptiScaler(c: OptiScalerConfig): string {
  const parts: string[] = [];

  if (c.dx12Upscaler) parts.push(`Upscalers.Dx12Upscaler=${c.dx12Upscaler}`);
  if (c.dx11Upscaler) parts.push(`Upscalers.Dx11Upscaler=${c.dx11Upscaler}`);
  if (c.vulkanUpscaler) parts.push(`Upscalers.VulkanUpscaler=${c.vulkanUpscaler}`);

  if (c.outputScalingOn) {
    parts.push("OutputScaling.Enabled=true");
    if (c.outputScalingMult.trim()) parts.push(`OutputScaling.Multiplier=${c.outputScalingMult.trim()}`);
  }

  if (c.sharpenOn) {
    parts.push("Sharpness.OverrideSharpness=true");
    if (c.sharpenShader) parts.push(`Sharpness.Shader=${c.sharpenShader}`);
    if (c.sharpness.trim()) parts.push(`Sharpness.Sharpness=${c.sharpness.trim()}`);
  }

  if (c.frameGenOn) {
    parts.push("FrameGen.Enabled=true");
    if (c.fgInput) parts.push(`FrameGen.FGInput=${c.fgInput}`);
    if (c.fgOutput) parts.push(`FrameGen.FGOutput=${c.fgOutput}`);
  }

  if (c.dlssPresetOn) {
    parts.push("DLSS.RenderPresetOverride=true");
    if (c.dlssPreset.trim()) parts.push(`DLSS.RenderPresetForAll=${c.dlssPreset.trim()}`);
  }

  return parts.join(";");
}
