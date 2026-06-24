import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl as openExternal } from "@tauri-apps/plugin-opener";
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
 * Mirror of hardware.rs `irrelevance`: a reason an option doesn't apply here.
 * `hdr` is not backend-detectable, so it's an opt-in capability carried on the
 * hardware object the UI passes in (sourced from the Settings toggle).
 */
export function irrelevance(
  hw: Hardware & { hdr?: boolean },
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
  }
  return null;
}

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
