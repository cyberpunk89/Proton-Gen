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

/**
 * Compatibility fixes, described by the symptom rather than the ini key.
 *
 * These are the settings people actually reach for when a game misbehaves, and
 * every one of them is documented upstream against a *symptom*, not a feature —
 * so that is how they're labelled here. Nobody types "RestoreComputeSignature"
 * into a search box; they search for the game that crashed.
 *
 * Each fix is one or more `Section.Key=Value` pairs, applied together and
 * detected as "on" when the first pair matches. Keys are from the OptiScaler.ini
 * reference (github.com/optiscaler/OptiScaler) — if proton-cachyos ships a build
 * that moves one between sections, it fails silently, so the pairs are kept
 * here in one list rather than spread through the component.
 */
export interface Fix {
  id: string;
  /** The symptom, as the user would describe it. */
  label: string;
  /** Which games or engines this usually affects. */
  note: string;
  pairs: [string, string][];
}

export const OPTI_FIXES: Fix[] = [
  {
    id: "reEngine",
    label: "Crashes or hangs in RE Engine games",
    note: "Resident Evil, Monster Hunter, Devil May Cry 5 — especially with DLSS inputs.",
    pairs: [
      ["Hotfix.RestoreComputeSignature", "true"],
      ["Hotfix.RestoreGraphicSignature", "true"],
    ],
  },
  {
    id: "colorBarrier",
    label: "Coloured blocks or bands at the screen edge",
    note: "Common in Unreal Engine titles: the DLSS plugin hands over resources in the wrong state.",
    pairs: [["Hotfix.ColorResourceBarrier", "4"]],
  },
  {
    id: "autoExposure",
    label: "Crushed blacks, or a white screen",
    note: "The game's exposure texture isn't usable; compute it instead.",
    pairs: [["InitFlags.AutoExposure", "true"]],
  },
  {
    id: "xessPipelines",
    label: "XeSS crashes, or slows down the longer you play",
    note: "Skips XeSS pipeline pre-building. Mostly older GPUs.",
    pairs: [["XeSS.BuildPipelines", "false"]],
  },
  {
    id: "motionBlur",
    label: "Excessive motion blur",
    note: "The game reports display-resolution motion vectors when it shouldn't.",
    pairs: [["InitFlags.DisplayResolution", "true"]],
  },
  {
    id: "arcSpoofing",
    label: "Rainbow artifacts on an Intel Arc GPU",
    note: "Turns off DXGI spoofing, which Arc doesn't tolerate when posing as NVIDIA.",
    pairs: [["Spoofing.Dxgi", "false"]],
  },
];

/** [PROTON_OPTISCALER_NAME] the DLL OptiScaler injects as. Not part of the ini —
 *  it is its own env var, but it belongs next to the upscaler pickers because
 *  it is the first thing to change when injection doesn't happen at all. */
export const PROXY_DLLS: Choice[] = [
  { value: "", label: "Default (dxgi.dll)" },
  { value: "dxgi.dll", label: "dxgi.dll" },
  { value: "d3d12.dll", label: "d3d12.dll" },
  { value: "dbghelp.dll", label: "dbghelp.dll" },
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
  /** Which entries of `OPTI_FIXES` are on, keyed by `Fix.id`. */
  fixes: Record<string, boolean>;
  /**
   * `Section.Key=Value` entries this builder doesn't model, preserved verbatim.
   *
   * Without this the round-trip is lossy in the one direction that costs real
   * work: opening the dialog on a config someone hand-tuned (or copied from a
   * game's wiki page) and pressing Apply would silently delete every key
   * outside the whitelist. The upstream ini has hundreds of keys and this
   * builder covers a couple of dozen, so that is the common case, not the edge.
   */
  passthrough: string[];
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
    fixes: {},
    passthrough: [],
  };
}

/** Every `Section.Key` the builder models, lowercased — anything else is
 *  passthrough. Derived from the fix table so the two can't drift. */
const KNOWN_KEYS = new Set(
  [
    "upscalers.dx12upscaler",
    "upscalers.dx11upscaler",
    "upscalers.vulkanupscaler",
    "outputscaling.enabled",
    "outputscaling.multiplier",
    "sharpness.overridesharpness",
    "sharpness.shader",
    "sharpness.sharpness",
    "framegen.enabled",
    "framegen.fginput",
    "framegen.fgoutput",
    "dlss.renderpresetoverride",
    "dlss.renderpresetforall",
    ...OPTI_FIXES.flatMap((f) => f.pairs.map(([k]) => k)),
  ].map((k) => k.toLowerCase()),
);

/**
 * Parse a `PROTON_OPTISCALER_CONFIG` string into the builder's state.
 *
 * Tolerant on purpose: section/key matching is case-insensitive and whitespace
 * around tokens is trimmed. Entries outside the modelled set are kept in
 * `passthrough` and re-emitted unchanged rather than dropped, so opening this
 * dialog can never destroy a config someone wrote by hand.
 */
export function parseOptiScaler(str: string): OptiScalerConfig {
  const c = emptyOptiScaler();
  const map = new Map<string, string>();
  for (const entry of str.split(";")) {
    const eq = entry.indexOf("=");
    if (eq < 0) continue;
    const key = entry.slice(0, eq).trim();
    const val = entry.slice(eq + 1).trim();
    if (!key) continue;
    const lower = key.toLowerCase();
    if (KNOWN_KEYS.has(lower)) map.set(lower, val);
    // Original casing kept: these go back out untouched.
    else c.passthrough.push(`${key}=${val}`);
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

  // A fix is on when its first pair is present with the expected value. Only the
  // first is checked: the pairs are applied together, so a partial match means
  // someone edited it by hand and the checkbox should reflect their intent to
  // have the fix rather than silently un-tick and drop the rest.
  for (const f of OPTI_FIXES) {
    const [key, want] = f.pairs[0];
    const got = get(key);
    if (got !== undefined && got.toLowerCase() === want.toLowerCase()) c.fixes[f.id] = true;
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

  for (const f of OPTI_FIXES) {
    if (!c.fixes[f.id]) continue;
    for (const [k, v] of f.pairs) parts.push(`${k}=${v}`);
  }

  // Last, so the builder's own output stays in a stable, readable order and the
  // foreign keys are visibly a tail rather than interleaved.
  parts.push(...c.passthrough);

  return parts.join(";");
}
