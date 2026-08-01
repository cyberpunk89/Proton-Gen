import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl as openExternal } from "@tauri-apps/plugin-opener";
import { inTauri } from "./ipc";
import type { Hardware } from "./types";

export async function copyText(text: string) {
  try {
    await writeText(text);
  } catch {
    // Fallback for browser/dev preview.
    await navigator.clipboard?.writeText(text);
  }
}

export async function openUrl(url: string) {
  try {
    await openExternal(url);
  } catch {
    window.open(url, "_blank");
  }
}

/**
 * Steam's Properties dialog for a game. Its General tab holds Launch Options —
 * the exact place the built command gets pasted.
 *
 * If Valve has renamed this verb (they have changed protocol verbs before),
 * swapping this for `steamLibraryUrl` is the one-line fix: the library page is
 * two clicks from Properties and far less likely to have moved.
 */
export function steamPropertiesUrl(appId: number): string {
  return `steam://gameproperties/${appId}`;
}

/** A game's library page. The conservative fallback for the above. */
export function steamLibraryUrl(appId: number): string {
  return `steam://nav/games/details/${appId}`;
}

/**
 * Hand a `steam://` URL to the system handler. Returns false when it could not
 * be handed off, so the caller can explain instead of appearing to do nothing.
 *
 * Deliberately does *not* fall back to `window.open` the way `openUrl` does: a
 * custom scheme in a dev browser either throws or silently no-ops, and "silently
 * no-ops" is the one outcome worth avoiding. `open::that_detached` on the Rust
 * side spawns without waiting, so a handler that starts and then fails is
 * invisible to us — this only catches a rejected invoke (missing handler, or a
 * capability-scope regression).
 */
export async function openSteamUrl(url: string): Promise<boolean> {
  if (!inTauri) return false;
  try {
    await openExternal(url);
    return true;
  } catch (e) {
    console.error("openSteamUrl failed", url, e);
    return false;
  }
}

/**
 * Mirror of hardware.rs `irrelevance`: a reason an option doesn't apply here.
 * `hdr` is not backend-detectable, so it's an opt-in capability carried on the
 * hardware object the UI passes in (sourced from the Settings toggle).
 */
export function irrelevance(
  hw: Hardware & { hdr?: boolean; fsr4?: boolean },
  gpu: string | null,
  needs: string[],
): string | null {
  if (gpu) {
    const g = gpu.toLowerCase();
    if (g === "nvidia" && !hw.nvidia) return "needs NVIDIA GPU";
    if (g === "amd" && !hw.amd) return "needs AMD GPU";
    if (g === "intel" && !hw.intel) return "needs Intel GPU";
  }
  for (const n of needs) {
    if (n === "wayland" && !hw.wayland) return "needs Wayland session";
    if (n === "kde" && !hw.kde) return "needs KDE Plasma";
    if (n === "ntsync" && !hw.ntsync) return "needs /dev/ntsync";
    if (n === "hdr" && !hw.hdr) return "needs HDR display";
    if (n === "fsr4" && !hw.fsr4) return "needs an RDNA3/RDNA4 GPU (FSR upgrades)";
  }
  return null;
}

/**
 * @deprecated Naive substring OR — no ranking and no highlight positions.
 * Prefer `fuzzy()` from `$lib/fuzzy` for anything user-facing. Still used by
 * the param list until that search is reworked.
 */
export function matches(filter: string, haystack: string[]): boolean {
  const f = filter.trim().toLowerCase();
  if (!f) return true;
  return haystack.some((h) => h.toLowerCase().includes(f));
}

/**
 * Quote-aware split of a `K=V K=V …` string, mirroring `parser::tokenize`
 * (src-tauri/src/parser.rs) so `FOO="a b"` stays one token.
 *
 * Kept in lockstep with the Rust side deliberately: the "custom env" chips in
 * ActiveOptions must agree with what `compose::parse_extra_env` will actually
 * put on the command line, or removing a chip would edit a token the backend
 * never saw.
 */
export function tokenizeEnv(input: string): string[] {
  const tokens: string[] = [];
  let cur = "";
  let quote: string | null = null;
  let has = false;
  for (const ch of input) {
    if (quote !== null) {
      if (ch === quote) quote = null;
      else cur += ch;
      continue;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
      has = true;
    } else if (/\s/.test(ch)) {
      if (has) {
        tokens.push(cur);
        cur = "";
        has = false;
      }
    } else {
      cur += ch;
      has = true;
    }
  }
  if (has) tokens.push(cur);
  return tokens;
}

/** `K=V` pairs from the custom-env field, alongside the raw token so a caller
 *  can remove exactly what it displayed. */
export function splitExtraEnv(input: string): { raw: string; key: string; value: string }[] {
  return tokenizeEnv(input).flatMap((raw) => {
    const at = raw.indexOf("=");
    if (at < 0) return [];
    return [{ raw, key: raw.slice(0, at), value: raw.slice(at + 1) }];
  });
}

/**
 * ProtonDB tier colours. Deliberately *not* theme tokens — these are the
 * medal colours the tiers are named after, and they have to mean the same
 * thing on every theme.
 */
const TIER_COLORS: Record<string, string> = {
  platinum: "#b4c7d9",
  gold: "#cfb53b",
  silver: "#a6a6a6",
  bronze: "#cd7f32",
  borked: "#e06c75",
};

const TIER_FALLBACK = "#909090";

/** The two inks a tier pill can use. Fixed, for the same reason as above: they
 *  sit on a fixed background, so a theme token would be the wrong contrast. */
const TIER_INK_DARK = "#11111b";
const TIER_INK_LIGHT = "#f4f4f8";

export function tierColor(tier: string): string {
  return TIER_COLORS[tier] ?? TIER_FALLBACK;
}

/** WCAG relative luminance of a `#rrggbb` colour. */
function luminance(hex: string): number {
  const h = hex.replace(/^#/, "");
  const ch = [0, 2, 4].map((i) => {
    const c = (parseInt(h.slice(i, i + 2), 16) || 0) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * ch[0] + 0.7152 * ch[1] + 0.0722 * ch[2];
}

const contrast = (a: number, b: number) => (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);

/**
 * Readable foreground for a tier pill, derived from `tierColor` rather than
 * hardcoded. Today every tier is light enough that the dark ink wins, so this
 * changes nothing visually — the point is that adding or restyling a tier can
 * no longer silently produce an unreadable pill.
 */
export function tierForeground(tier: string): string {
  const bg = luminance(tierColor(tier));
  const dark = luminance(TIER_INK_DARK);
  const light = luminance(TIER_INK_LIGHT);
  return contrast(bg, dark) >= contrast(bg, light) ? TIER_INK_DARK : TIER_INK_LIGHT;
}

/** Best-to-worst, so trending/best can be compared against the overall tier.
 *  An unknown tier sorts below `borked` and is treated as "no signal". */
const TIER_ORDER = ["platinum", "gold", "silver", "bronze", "borked"];

/** Lower is better. `null` when the tier isn't one ProtonDB ranks. */
export function tierRank(tier: string): number | null {
  const i = TIER_ORDER.indexOf(tier);
  return i === -1 ? null : i;
}
