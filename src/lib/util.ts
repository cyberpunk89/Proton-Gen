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

const TIER_COLORS: Record<string, string> = {
  platinum: "#b4c7d9",
  gold: "#cfb53b",
  silver: "#a6a6a6",
  bronze: "#cd7f32",
  borked: "#e06c75",
};

export function tierColor(tier: string): string {
  return TIER_COLORS[tier] ?? "#909090";
}
