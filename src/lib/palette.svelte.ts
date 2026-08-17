import type { Component } from "svelte";
import { GameController, Faders, Sparkle, FloppyDisk, Package } from "phosphor-svelte";
import { computeCommandScore } from "bits-ui";

import { app } from "./state.svelte";
import { toast } from "./toast.svelte";
import { irrelevance } from "./util";
import { subsequence } from "./fuzzy";
import { APP_COMMANDS } from "./commands";

export type PaletteGroup = "action" | "param" | "recipe" | "game" | "preset";

export interface PaletteItem {
  id: string;
  group: PaletteGroup;
  label: string;
  sublabel?: string;
  /** Extra match text, already lowercased. */
  keywords: string[];
  icon?: Component;
  badge?: string;
  /** Why this is a poor fit right now. Demotes, never hides. */
  dimReason?: string | null;
  run: () => void | Promise<void>;
  runAlt?: () => void;
  altLabel?: string;
}

const GROUP_LABEL: Record<PaletteGroup, string> = {
  action: "Actions",
  param: "Parameters",
  recipe: "Recipes",
  game: "Games",
  preset: "Presets",
};

/** Nudges ties toward what a palette user most often wants. */
const GROUP_WEIGHT: Record<PaletteGroup, number> = {
  action: 1.15,
  param: 1.0,
  game: 1.0,
  recipe: 0.95,
  preset: 0.95,
};

/** Hard caps, so a 5000-game library can never put 5000 nodes in the DOM. */
const CAPS: Record<PaletteGroup, number> = {
  game: 8,
  param: 8,
  recipe: 5,
  action: 6,
  preset: 4,
};

export interface PaletteSection {
  group: PaletteGroup;
  label: string;
  items: PaletteItem[];
  /** Matches before the cap, so the heading can say "8 of 214". */
  total: number;
  best: number;
}

/**
 * Everything the palette can reach, built **once per open** rather than per
 * keystroke.
 *
 * `irrelevance()` runs here, in this one pass, for the same reason #49 collapsed
 * MainPanel's four passes into one: re-deriving it for 87 params on every
 * keystroke is exactly the waste that makes a palette feel heavy.
 */
export function buildItems(): PaletteItem[] {
  const items: PaletteItem[] = [];

  for (const c of APP_COMMANDS) {
    const ok = c.available?.() ?? true;
    items.push({
      id: `action:${c.id}`,
      group: "action",
      label: c.label,
      keywords: (c.keywords ?? []).map((k) => k.toLowerCase()),
      icon: c.icon,
      dimReason: ok ? null : "not available right now",
      run: c.run,
    });
  }

  for (const g of app.games) {
    items.push({
      id: `game:${g.source}:${g.app_id}`,
      group: "game",
      label: g.name,
      sublabel:
        g.source === "non-steam" ? "shortcut" : g.source === "heroic" ? "Heroic" : undefined,
      keywords: [String(g.app_id)],
      icon: GameController,
      badge: app.isFavorite(g.app_id) ? "★" : undefined,
      dimReason: g.installed ? null : "not installed",
      run: () => app.openGame(g),
    });
  }

  // Every parameter is listed here, including `tier = "advanced"` ones the
  // panel hides. Deliberate: the palette is how you reach something by name, and
  // filtering it by a tidiness preference would make advanced options
  // unreachable rather than merely tucked away. `revealParam` flips
  // `show_advanced` on the way in, so the row is actually there when you land.
  for (const e of app.catalog.envs) {
    const enabled = app.env[e.key]?.enabled ?? false;
    items.push({
      id: `param:env:${e.key}`,
      group: "param",
      label: e.key,
      sublabel: e.category,
      keywords: [e.category.toLowerCase(), e.help.toLowerCase()],
      icon: Faders,
      badge: enabled ? "on" : undefined,
      dimReason: irrelevance(app.hwCaps, e.gpu, e.needs),
      run: () => {
        app.revealParam(e.key);
      },
      // Ctrl+Enter toggles without leaving the palette, so you can flip three
      // options without three round trips through the panel.
      runAlt: () => app.toggleEnv(e.key),
      altLabel: enabled ? "disable" : "enable",
    });
  }

  for (const w of app.catalog.wrappers) {
    const enabled = app.wrap[w.key]?.enabled ?? false;
    items.push({
      id: `param:wrap:${w.key}`,
      group: "param",
      label: w.label ?? w.key,
      sublabel: "Wrappers",
      keywords: [w.key.toLowerCase(), w.help.toLowerCase()],
      icon: Package,
      badge: enabled ? "on" : undefined,
      dimReason: irrelevance(app.hwCaps, w.gpu, w.needs),
      run: () => {
        app.revealParam(w.key);
      },
      runAlt: () => app.toggleWrap(w.key),
      altLabel: enabled ? "disable" : "enable",
    });
  }

  app.recipes.forEach((r, i) => {
    items.push({
      id: `recipe:${r.name}`,
      group: "recipe",
      label: r.name,
      sublabel: r.description || undefined,
      keywords: r.tags.map((t) => t.toLowerCase()),
      icon: Sparkle,
      dimReason: irrelevance(app.hwCaps, r.gpu === "any" ? null : r.gpu, r.needs),
      async run() {
        await app.applyRecipe(i);
        toast.success(`Applied: ${r.name}`);
      },
    });
  });

  for (const p of app.store.presets) {
    items.push({
      id: `preset:${p.name}`,
      group: "preset",
      label: p.name,
      sublabel: p.game_name ?? undefined,
      keywords: [],
      icon: FloppyDisk,
      run: () => app.loadPreset(p.name),
    });
  }

  return items;
}

/** Precomputed lowercase haystack, so ranking never lowercases in the loop. */
interface Indexed {
  item: PaletteItem;
  hay: string;
}

export function indexItems(items: PaletteItem[]): Indexed[] {
  return items.map((item) => ({ item, hay: item.label.toLowerCase() }));
}

/**
 * Rank and cap.
 *
 * `subsequence()` is the gate: it discards the overwhelming majority of a large
 * library in microseconds, so `computeCommandScore` — much more expensive — only
 * ever runs on survivors. No debounce: a debounced palette feels broken, so the
 * work is capped instead.
 */
export function rank(indexed: Indexed[], query: string): PaletteSection[] {
  const q = query.trim().toLowerCase();
  const buckets = new Map<PaletteGroup, { scored: { item: PaletteItem; score: number }[] }>();

  for (const { item, hay } of indexed) {
    let score: number;

    if (!q) {
      // Empty query is a curated view, not a dump: actions first, then things
      // already in play (enabled params, favourites), then the rest.
      score =
        (item.group === "action" ? 3 : 0) +
        (item.badge === "on" ? 2 : 0) +
        (item.badge === "★" ? 1.5 : 0) +
        (item.group === "recipe" ? 1 : 0);
      if (score === 0) continue;
    } else {
      const direct = subsequence(hay, q);
      const viaKeyword = direct ? null : item.keywords.find((k) => subsequence(k, q));
      if (!direct && !viaKeyword) continue;

      score = computeCommandScore(item.label, query, item.keywords);
      // computeCommandScore returns 0 for multi-word queries; floor matched
      // candidates above zero so they still outrank non-matches.
      if (score <= 0) score = Number.EPSILON;
      if (hay.startsWith(q)) score *= 1.5;
      if (item.badge === "on") score *= 1.05;
    }

    // Demote, never hide. Hiding an option in a palette reads as a bug — the
    // same reason MainPanel dims irrelevant rows rather than dropping them.
    if (item.dimReason) score *= 0.5;

    const b = buckets.get(item.group) ?? { scored: [] };
    b.scored.push({ item, score });
    buckets.set(item.group, b);
  }

  const sections: PaletteSection[] = [];
  for (const [group, { scored }] of buckets) {
    scored.sort((a, b) => b.score - a.score);
    sections.push({
      group,
      label: GROUP_LABEL[group],
      items: scored.slice(0, CAPS[group]).map((s) => s.item),
      total: scored.length,
      best: (scored[0]?.score ?? 0) * GROUP_WEIGHT[group],
    });
  }
  return sections.sort((a, b) => b.best - a.best);
}
