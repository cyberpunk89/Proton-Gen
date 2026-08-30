import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl as openExternal } from "@tauri-apps/plugin-opener";
import { inTauri } from "./ipc";
import type { GameDto, Hardware, HwCaps, Tier } from "./types";

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
 * A reason an option doesn't apply on this machine, or null if it does.
 *
 * This is the *only* relevance filter in the app — there is deliberately no Rust
 * mirror. `hardware.rs` used to carry one and it rotted into dead code three
 * tags behind, because the opt-in capabilities (`hdr`, `fsr4`, `rdna3`,
 * `rdna4`) live in the frontend store and never cross the IPC boundary.
 *
 * Unknown tags fall through as relevant, on purpose: `params.toml` is
 * overridable from `$XDG_CONFIG_HOME`, so a user copy naming a capability a
 * future build adds must still load rather than silently hiding rows. The
 * *shipped* TOML is held to the known set by a Rust test instead
 * (`params::tests::bundled_needs_tags_are_known`).
 */
export function irrelevance(
  hw: Partial<HwCaps> & Hardware,
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
    // The two generations are mutually exclusive and both must filter, or the
    // Settings selector is decorative: before `rdna4` existed, every RDNA4-only
    // recipe stayed visible on RDNA3 because an unhandled tag reads as relevant.
    if (n === "rdna3" && !hw.rdna3) return "RDNA3-only";
    if (n === "rdna4" && !hw.rdna4) return "RDNA4-only";
  }
  return null;
}

/**
 * Whether any of an entry's `recommended_for` capability tags is currently
 * true, per the same `hw`/`HwCaps` bag `irrelevance()` reads. Empty always
 * reads as false ("not tagged as recommended for anything") — this is a
 * positive match, not a fallback-permissive filter like `irrelevance()`.
 */
export function isRecommended(hw: Partial<HwCaps> & Hardware, recommendedFor: string[]): boolean {
  return recommendedFor.some((tag) => {
    switch (tag) {
      case "wayland":
        return hw.wayland;
      case "kde":
        return hw.kde;
      case "ntsync":
        return hw.ntsync;
      case "hdr":
        return !!hw.hdr;
      case "fsr4":
        return !!hw.fsr4;
      case "rdna3":
        return !!hw.rdna3;
      case "rdna4":
        return !!hw.rdna4;
      default:
        return false;
    }
  });
}

/**
 * Whether a recipe's `protondb_tiers` hint matches the selected game's
 * fetched ProtonDB tier.
 *
 * Kept separate from `irrelevance()` on purpose: a tier is async, per-game,
 * opt-in network data that may not be fetched yet (`undefined`) or may have
 * failed (`null`), whereas `irrelevance()`'s contract is static hardware/
 * session state available synchronously for every recipe at once. An empty
 * `protondb_tiers` means "not tier-specific" — always matches, including when
 * no tier is available, so recipes with no tier hint are never affected by
 * this filter.
 */
export function matchesTier(tiers: string[], tier: Tier | null | undefined): boolean {
  if (!tiers.length) return true;
  if (!tier) return false;
  return tiers.includes(tier.tier.toLowerCase());
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
 * Render pairs back into the custom-env field's `K=V K=V …` form, re-quoting any
 * value containing whitespace. Mirrors `compose::format_extra_env`
 * (src-tauri/src/compose.rs) and is the exact inverse of `tokenizeEnv`.
 */
export function formatExtraEnv(pairs: [string, string][]): string {
  return pairs.map(([k, v]) => (/\s/.test(v) ? `${k}="${v}"` : `${k}=${v}`)).join(" ");
}

/**
 * Append `pairs` to an extra-env string, skipping any key it already assigns.
 * Mirrors `compose::merge_into_extra_env` (src-tauri/src/compose.rs), including
 * the tie-break: dedup is **by key and the incoming pair loses**, because these
 * pairs carry values from a catalog the app no longer has, and what the user
 * typed into the visible field outranks that.
 */
export function mergeIntoExtraEnv(extraEnv: string, pairs: [string, string][]): string {
  const existing = new Set(splitExtraEnv(extraEnv).map((p) => p.key));
  const fresh = pairs.filter(([k]) => !existing.has(k));
  if (fresh.length === 0) return extraEnv;
  const rendered = formatExtraEnv(fresh);
  return extraEnv.trim() === "" ? rendered : `${extraEnv.trimEnd()} ${rendered}`;
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

/** One library tile's worth of games: usually one, or several that
 *  `groupGames` folded together because they're the same title on different
 *  sources. */
export interface GameGroup {
  key: string;
  entries: GameDto[];
}

/** steam > non-steam > heroic, so a merged tile's badges/favourite/tuned
 *  status (keyed off `entries[0]`) prefer the Steam entry when there is one —
 *  that's the entry with real launch-options sync status to show. */
const SOURCE_PRIORITY: Record<string, number> = { steam: 0, "non-steam": 1, heroic: 2 };

/**
 * Normalize a game title for cross-source matching. Lowercased, diacritics
 * stripped, punctuation/whitespace collapsed to single spaces. Deliberately
 * loose rather than exact: the same game sideloaded into Heroic often has
 * different punctuation or casing than its Steam listing (curly vs straight
 * apostrophe, a trailing edition suffix typed by hand when it was added).
 */
export function normalizeGameName(name: string): string {
  return name
    .normalize("NFKD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

/**
 * Group games that share a normalized title across sources — the "installed
 * both via Steam and sideloaded in Heroic" case the library grid folds into
 * one tile with a source picker rather than showing twice.
 *
 * Order is preserved: each group lands at its first member's position in
 * `games` (so the caller's sort — favourites, recent-first, alphabetical —
 * still determines where a merged tile sits), and entries within a group are
 * sorted by `SOURCE_PRIORITY`.
 */
export function groupGames(games: GameDto[]): GameGroup[] {
  const bucket = new Map<string, GameDto[]>();
  for (const g of games) {
    const key = normalizeGameName(g.name);
    const arr = bucket.get(key);
    if (arr) arr.push(g);
    else bucket.set(key, [g]);
  }

  const seen = new Set<string>();
  const out: GameGroup[] = [];
  for (const g of games) {
    const key = normalizeGameName(g.name);
    if (seen.has(key)) continue;
    seen.add(key);
    const entries = [...bucket.get(key)!].sort(
      (a, b) => (SOURCE_PRIORITY[a.source] ?? 9) - (SOURCE_PRIORITY[b.source] ?? 9),
    );
    out.push({ key, entries });
  }
  return out;
}

/**
 * Combine the inline `style` bits-ui hands us through a `child` snippet with our
 * own declarations. Use this at EVERY `{...props}` site that also needs an inline
 * style — a bare `style="…"` written after the spread is a silent P0.
 *
 * While a modal layer is open bits-ui sets `document.body { pointer-events: none }`
 * and re-enables the panel with `pointer-events: auto`, which it delivers *inside*
 * `props.style` — as a string, because svelte-toolbelt's `mergeProps` stringifies
 * its style object. A literal `style=` after `{...props}` is a later key in the
 * same compiled object literal, so it replaces that string wholesale;
 * `pointer-events` inherits, and the panel plus every control in it goes
 * click-dead with no error, no warning and a clean `pnpm check`. That shipped
 * once (#63) and took a bug report to find.
 *
 * The Content props also carry `FocusScope`'s `onkeydown` (the Tab focus trap) and,
 * on `Select.Content`, load-bearing layout style — so the same rule covers event
 * handlers. Enforced by `scripts/check-props-spread.sh`, which `pnpm check` runs.
 *
 * `props` is typed `Record<string, unknown>` by bits-ui, hence the `unknown`.
 * Inputs may or may not carry a trailing `;`, so it is normalised here.
 */
export function mergeStyle(
  props: { style?: unknown },
  ...extra: (string | false | null | undefined)[]
): string {
  const parts = [props.style, ...extra]
    .filter((s): s is string => typeof s === "string" && s.trim() !== "")
    .map((s) => s.trim().replace(/;+$/, ""));
  return parts.length === 0 ? "" : `${parts.join("; ")};`;
}
