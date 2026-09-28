/**
 * vkBasalt effect chain: the real `vkBasalt.conf` text <-> UI state round-trip.
 *
 * Unlike MANGOHUD_CONFIG/PROTON_OPTISCALER_CONFIG, vkBasalt has no inline
 * env-var carrier — its only config surface is the real file, always
 * `key = value` (spaces around `=`, no bare tokens, no comma-separation).
 * This still mirrors `optiscaler.ts`'s shape: pure string/struct functions
 * with no Svelte dependency, a `passthrough: string[]` bucket for anything
 * this builder doesn't model, and a round-trip property (`parse(build(c))`
 * preserves everything the UI expresses) that only shows up when the two
 * sit next to each other.
 *
 * Keys and accepted values are from the vkBasalt.conf reference
 * (github.com/DadSchoorse/vkBasalt). Only the widely-used effects are
 * surfaced (CAS, FXAA, SMAA, DLS); `deband`, `lutFile` and custom ReShade
 * shaders are still reachable by hand-editing the file — this builder
 * preserves them untouched rather than modeling them.
 */

export interface Choice {
  value: string;
  label: string;
}

/** smaaEdgeDetection — vkBasalt only accepts these two; there is no `depth`. */
export const SMAA_EDGE_DETECTION: Choice[] = [
  { value: "luma", label: "Luma (default)" },
  { value: "color", label: "Color" },
];

/** Everything the vkBasalt builder can express, as plain data. */
export interface VkBasaltConfig {
  casOn: boolean;
  casSharpness: string; // -1.0 - 1.0, default 0.4
  dlsOn: boolean;
  dlsSharpness: string; // 0.0 - 1.0, default 0.5
  dlsDenoise: string; // 0.0 - 1.0, default 0.17
  fxaaOn: boolean;
  fxaaQualitySubpix: string; // 0.00 - 1.00, default 0.75
  fxaaQualityEdgeThreshold: string; // 0.063 - 0.333, default 0.125
  fxaaQualityEdgeThresholdMin: string; // 0.0312 - 0.0833, default 0.0312
  smaaOn: boolean;
  smaaEdgeDetection: string; // luma | color
  smaaThreshold: string; // 0 - 0.5, default 0.05
  smaaMaxSearchSteps: string; // 0 - 112, default 32
  smaaMaxSearchStepsDiag: string; // 0 - 20, default 16
  smaaCornerRounding: string; // 0 - 100, default 25
  /** An X11 keysym NAME (`Home`, `Prior`, `F12`), not a VK code like
   *  OptiScaler's. Empty leaves vkBasalt's own default (Home) in place. */
  toggleKey: string;
  /** vkBasalt's own default is already `true`; this only ever needs to be
   *  emitted when explicitly turned off. */
  enableOnLaunch: boolean;
  /**
   * Tokens in the `effects=` line this builder doesn't model — `deband`,
   * `lut`, a custom ReShade effect id — kept in the chain's original
   * position relative to the modeled ones isn't preserved (they're always
   * appended after), but the tokens themselves are never dropped.
   */
  effectsPassthrough: string[];
  /**
   * `key = value` lines this builder doesn't model, preserved verbatim —
   * `lutFile`, `deband*` tuning, `reshadeTexturePath`/`reshadeIncludePath`, a
   * custom ReShade effect's own shader-path line, and comments.
   *
   * Without this the round-trip is lossy in the one direction that costs
   * real work: opening the dialog on a file someone hand-tuned (or copied
   * from a wiki page) and pressing Apply would silently delete every setting
   * outside the whitelist.
   */
  passthrough: string[];
}

export function emptyVkBasalt(): VkBasaltConfig {
  return {
    casOn: false,
    casSharpness: "0.4",
    dlsOn: false,
    dlsSharpness: "0.5",
    dlsDenoise: "0.17",
    fxaaOn: false,
    fxaaQualitySubpix: "0.75",
    fxaaQualityEdgeThreshold: "0.125",
    fxaaQualityEdgeThresholdMin: "0.0312",
    smaaOn: false,
    smaaEdgeDetection: "luma",
    smaaThreshold: "0.05",
    smaaMaxSearchSteps: "32",
    smaaMaxSearchStepsDiag: "16",
    smaaCornerRounding: "25",
    toggleKey: "",
    enableOnLaunch: true,
    effectsPassthrough: [],
    passthrough: [],
  };
}

/** Every key the builder models, exact-case (vkBasalt keys are
 *  case-sensitive camelCase, unlike OptiScaler's lowercased ini keys) —
 *  anything else is passthrough. */
const KNOWN_KEYS = new Set([
  "effects",
  "casSharpness",
  "dlsSharpness",
  "dlsDenoise",
  "fxaaQualitySubpix",
  "fxaaQualityEdgeThreshold",
  "fxaaQualityEdgeThresholdMin",
  "smaaEdgeDetection",
  "smaaThreshold",
  "smaaMaxSearchSteps",
  "smaaMaxSearchStepsDiag",
  "smaaCornerRounding",
  "toggleKey",
  "enableOnLaunch",
]);

/** Built-in effect ids this builder has a checkbox for. */
const MODELED_EFFECTS = new Set(["cas", "dls", "fxaa", "smaa"]);

/**
 * Parse a `vkBasalt.conf`-shaped string into the builder's state.
 *
 * Case-sensitive key matching (unlike `optiscaler.ts`'s lowercased matching —
 * vkBasalt's own parser is case-sensitive, so a config with the wrong case
 * simply wouldn't work for vkBasalt either). Comments and blank lines are
 * dropped on parse and never reconstructed; every other unrecognized line is
 * kept in `passthrough` and re-emitted unchanged.
 */
export function parseVkBasalt(text: string): VkBasaltConfig {
  const c = emptyVkBasalt();
  const map = new Map<string, string>();
  for (const rawLine of text.split("\n")) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq < 0) continue;
    const key = line.slice(0, eq).trim();
    const val = line.slice(eq + 1).trim();
    if (!key) continue;
    if (KNOWN_KEYS.has(key)) map.set(key, val);
    else c.passthrough.push(`${key} = ${val}`);
  }

  const get = (k: string) => map.get(k);
  const bool = (v: string | undefined, fallback: boolean) =>
    v === undefined ? fallback : v.toLowerCase() === "true" || v === "1";

  const effects = (get("effects") ?? "").split(":").map((t) => t.trim()).filter(Boolean);
  for (const id of effects) {
    if (id === "cas") c.casOn = true;
    else if (id === "dls") c.dlsOn = true;
    else if (id === "fxaa") c.fxaaOn = true;
    else if (id === "smaa") c.smaaOn = true;
    else if (!MODELED_EFFECTS.has(id)) c.effectsPassthrough.push(id);
  }

  if (get("casSharpness")) c.casSharpness = get("casSharpness")!;
  if (get("dlsSharpness")) c.dlsSharpness = get("dlsSharpness")!;
  if (get("dlsDenoise")) c.dlsDenoise = get("dlsDenoise")!;
  if (get("fxaaQualitySubpix")) c.fxaaQualitySubpix = get("fxaaQualitySubpix")!;
  if (get("fxaaQualityEdgeThreshold")) c.fxaaQualityEdgeThreshold = get("fxaaQualityEdgeThreshold")!;
  if (get("fxaaQualityEdgeThresholdMin"))
    c.fxaaQualityEdgeThresholdMin = get("fxaaQualityEdgeThresholdMin")!;
  if (get("smaaEdgeDetection")) c.smaaEdgeDetection = get("smaaEdgeDetection")!;
  if (get("smaaThreshold")) c.smaaThreshold = get("smaaThreshold")!;
  if (get("smaaMaxSearchSteps")) c.smaaMaxSearchSteps = get("smaaMaxSearchSteps")!;
  if (get("smaaMaxSearchStepsDiag")) c.smaaMaxSearchStepsDiag = get("smaaMaxSearchStepsDiag")!;
  if (get("smaaCornerRounding")) c.smaaCornerRounding = get("smaaCornerRounding")!;
  if (get("toggleKey")) c.toggleKey = get("toggleKey")!;
  c.enableOnLaunch = bool(get("enableOnLaunch"), true);

  return c;
}

/**
 * Build a `vkBasalt.conf`-shaped string from the builder's state.
 *
 * Fixed chain order (CAS, DLS, FXAA, SMAA, then any unmodeled ids) rather
 * than letting the user reorder — simpler, and vkBasalt's own docs order
 * their example the same way. Only set options are emitted, so a config that
 * changes nothing is the empty string.
 */
export function buildVkBasalt(c: VkBasaltConfig): string {
  const chain: string[] = [];
  if (c.casOn) chain.push("cas");
  if (c.dlsOn) chain.push("dls");
  if (c.fxaaOn) chain.push("fxaa");
  if (c.smaaOn) chain.push("smaa");
  chain.push(...c.effectsPassthrough);

  const parts: string[] = [];
  if (chain.length) parts.push(`effects = ${chain.join(":")}`);

  if (c.casOn && c.casSharpness.trim()) parts.push(`casSharpness = ${c.casSharpness.trim()}`);

  if (c.dlsOn) {
    if (c.dlsSharpness.trim()) parts.push(`dlsSharpness = ${c.dlsSharpness.trim()}`);
    if (c.dlsDenoise.trim()) parts.push(`dlsDenoise = ${c.dlsDenoise.trim()}`);
  }

  if (c.fxaaOn) {
    if (c.fxaaQualitySubpix.trim()) parts.push(`fxaaQualitySubpix = ${c.fxaaQualitySubpix.trim()}`);
    if (c.fxaaQualityEdgeThreshold.trim())
      parts.push(`fxaaQualityEdgeThreshold = ${c.fxaaQualityEdgeThreshold.trim()}`);
    if (c.fxaaQualityEdgeThresholdMin.trim())
      parts.push(`fxaaQualityEdgeThresholdMin = ${c.fxaaQualityEdgeThresholdMin.trim()}`);
  }

  if (c.smaaOn) {
    parts.push(`smaaEdgeDetection = ${c.smaaEdgeDetection}`);
    if (c.smaaThreshold.trim()) parts.push(`smaaThreshold = ${c.smaaThreshold.trim()}`);
    if (c.smaaMaxSearchSteps.trim()) parts.push(`smaaMaxSearchSteps = ${c.smaaMaxSearchSteps.trim()}`);
    if (c.smaaMaxSearchStepsDiag.trim())
      parts.push(`smaaMaxSearchStepsDiag = ${c.smaaMaxSearchStepsDiag.trim()}`);
    if (c.smaaCornerRounding.trim())
      parts.push(`smaaCornerRounding = ${c.smaaCornerRounding.trim()}`);
  }

  if (c.toggleKey) parts.push(`toggleKey = ${c.toggleKey}`);
  if (!c.enableOnLaunch) parts.push("enableOnLaunch = False");

  // Last, so the builder's own output stays in a stable, readable order and
  // the foreign lines are visibly a tail rather than interleaved.
  parts.push(...c.passthrough);

  return parts.join("\n");
}
