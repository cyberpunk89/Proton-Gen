<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { irrelevance } from "$lib/util";
  import { fuzzy } from "$lib/fuzzy";
  import OptionRow from "./OptionRow.svelte";
  import Recipes from "./Recipes.svelte";
  import GameRuntimePanel from "./GameRuntimePanel.svelte";
  import ActiveOptions from "./ActiveOptions.svelte";
  import Dialog from "./Dialog.svelte";
  import MangoHud from "./MangoHud.svelte";
  import { MagnifyingGlass, Faders, Gauge } from "phosphor-svelte";
  import type { EnvDef, WrapperDef } from "$lib/types";

  /**
   * The overlay builder. Lives here rather than inside the row so it survives a
   * section change (or the row scrolling out of a search result) while open,
   * and so both of its entry points share one instance.
   */
  let mangoOpen = $state(false);

  let showAll = $derived(app.store.show_irrelevant);
  const q = $derived(app.paramQuery);
  const searching = $derived(q.trim().length > 0);

  /** A reason this option doesn't apply to the detected hardware, else null. */
  function reason(it: { gpu: string | null; needs: string[] }): string | null {
    return irrelevance(app.hwCaps, it.gpu, it.needs);
  }

  // Pick the value control by how many choices the option offers:
  //   0 → free text · 1 → none (the switch implies it) · 2 → segmented · 3+ → dropdown
  function envValueField(n: number): "none" | "text" | "segmented" | "select" {
    if (n === 0) return "text";
    if (n === 1) return "none";
    if (n === 2) return "segmented";
    return "select";
  }

  /** One entry per catalog item that survives the current view's filter. */
  interface Hit {
    kind: "env" | "wrap";
    def: EnvDef | WrapperDef;
    key: string;
    /** The visible label: the env key, or a wrapper's label. */
    label: string;
    group: string;
    score: number;
    titleRanges: [number, number][];
    helpRanges: [number, number][];
    /** Filtered out by hardware relevance unless "Show all" is on. */
    hidden: boolean;
  }

  /**
   * A single traversal of the catalog, replacing four independent passes that
   * each re-ran the same predicate over all 87 items on every keystroke — `envs`,
   * `wraps` and `hiddenCount` (twice) all derived it separately.
   *
   * Emits everything downstream needs: matches, scores, highlight ranges, and the
   * hidden flag, so `hiddenCount` is a filter over this array rather than another
   * two traversals.
   */
  let hits = $derived.by((): Hit[] => {
    const out: Hit[] = [];
    const query = q.trim();

    for (const e of app.catalog.envs) {
      if (!searching && e.category !== app.activeSection) continue;
      if (searching) {
        const t = fuzzy(e.key, query, [e.category]);
        const h = fuzzy(e.help, query);
        if (!t && !h) continue;
        out.push({
          kind: "env",
          def: e,
          key: e.key,
          label: e.key,
          group: e.category,
          // A title hit is worth more than a help hit: people search for the
          // variable they half-remember, not for prose about it.
          score: (t?.score ?? 0) * 2 + (h?.score ?? 0),
          titleRanges: t?.ranges ?? [],
          helpRanges: h?.ranges ?? [],
          hidden: !!reason(e),
        });
      } else {
        out.push({
          kind: "env",
          def: e,
          key: e.key,
          label: e.key,
          group: e.category,
          score: 0,
          titleRanges: [],
          helpRanges: [],
          hidden: !!reason(e),
        });
      }
    }

    for (const w of app.catalog.wrappers) {
      if (!searching && app.activeSection !== "Wrappers") continue;
      const label = w.label ?? w.key;
      if (searching) {
        const t = fuzzy(label, query, [w.key, "wrappers"]);
        const h = fuzzy(w.help, query);
        if (!t && !h) continue;
        out.push({
          kind: "wrap",
          def: w,
          key: w.key,
          label,
          group: "Wrappers",
          score: (t?.score ?? 0) * 2 + (h?.score ?? 0),
          titleRanges: t?.ranges ?? [],
          helpRanges: h?.ranges ?? [],
          hidden: !!reason(w),
        });
      } else {
        out.push({
          kind: "wrap",
          def: w,
          key: w.key,
          label,
          group: "Wrappers",
          score: 0,
          titleRanges: [],
          helpRanges: [],
          hidden: !!reason(w),
        });
      }
    }
    return out;
  });

  let visible = $derived(hits.filter((h) => showAll || !h.hidden));
  let hiddenCount = $derived(hits.filter((h) => h.hidden).length);
  let anyShown = $derived(visible.length > 0);

  /** Search results grouped by category, groups ordered by their best member. */
  let groups = $derived.by(() => {
    if (!searching) return [];
    const by = new Map<string, Hit[]>();
    for (const h of visible) {
      const list = by.get(h.group);
      if (list) list.push(h);
      else by.set(h.group, [h]);
    }
    return [...by.entries()]
      .map(([name, items]) => ({
        name,
        items: [...items].sort((a, b) => b.score - a.score),
        best: Math.max(...items.map((i) => i.score)),
      }))
      .sort((a, b) => b.best - a.best);
  });

  let title = $derived(searching ? `Results for “${q.trim()}”` : app.activeSection);
  let resultSummary = $derived(
    `${visible.length} match${visible.length === 1 ? "" : "es"} in ${groups.length} categor${
      groups.length === 1 ? "y" : "ies"
    }`,
  );

  /** Wrappers first in a category view, matching the previous ordering. */
  let sectionWraps = $derived(visible.filter((h) => h.kind === "wrap"));
  let sectionEnvs = $derived(visible.filter((h) => h.kind === "env"));
</script>

{#if searching}
  {@render optionsCard()}
{:else if app.activeSection === "recipes"}
  <Recipes />
{:else if app.activeSection === "game"}
  <GameRuntimePanel />
{:else if app.activeSection === "@active"}
  <!-- Must come before the final {:else}: falling through would render an empty
       options card, since no catalog category is named "@active". -->
  <ActiveOptions />
{:else}
  {@render optionsCard()}
{/if}

{#snippet optionsCard()}
  <section class="card p-4">
    <div class="mb-3 flex items-center gap-2.5">
      {#if searching}
        <MagnifyingGlass size={18} class="text-accent" />
      {:else}
        <Faders size={18} class="text-accent" />
      {/if}
      <h2 class="text-sm font-medium tracking-wide text-text">{title}</h2>
      {#if searching}
        <span class="text-xs text-muted">· {resultSummary}</span>
      {/if}
    </div>

    {#if hiddenCount > 0}
      <div class="mb-3 flex items-center gap-2 text-xs text-muted">
        {#if showAll}
          <span>Showing all, including unsupported options.</span>
          <button
            onclick={() => app.setShowIrrelevant(false)}
            class="font-medium text-accent hover:underline">Hide unsupported</button
          >
        {:else}
          <span>{hiddenCount} hidden for your hardware.</span>
          <button
            onclick={() => app.setShowIrrelevant(true)}
            class="font-medium text-accent hover:underline">Show all</button
          >
        {/if}
      </div>
    {/if}

    {#if !anyShown}
      <div class="flex flex-col items-center gap-2 py-12 text-center">
        <MagnifyingGlass size={26} class="text-muted" />
        <p class="text-sm text-muted">
          {searching
            ? `No parameters match “${q.trim()}”.`
            : "Nothing to show here for your hardware."}
        </p>
        {#if searching}
          <button
            onclick={() => (app.paramQuery = "")}
            class="text-xs text-accent hover:underline">Clear search</button
          >
        {/if}
      </div>
    {:else if searching}
      <!-- Grouped so a 40-result query reads as a map of the catalog rather than
           a flat wall. Groups ordered by their best member, items by score. -->
      <div class="space-y-4">
        {#each groups as g (g.name)}
          <div>
            <p class="mb-1 px-3 text-[11px] font-medium uppercase tracking-wider text-muted">
              {g.name}
            </p>
            <div class="space-y-0.5">
              {#each g.items as h (h.kind + h.key)}
                {@render row(h)}
              {/each}
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="space-y-0.5">
        {#each sectionWraps as h (h.key)}
          {@render row(h)}
        {/each}
        {#each sectionEnvs as h (h.key)}
          {@render row(h)}
        {/each}
      </div>
    {/if}
  </section>
{/snippet}

<!--
  Wired to the `mangohud` wrapper *and* the MANGOHUD_CONFIG env row: those are
  the two places someone looks for it, and neither is obviously the "real" one.
-->
{#snippet configureOverlay()}
  <button
    type="button"
    onclick={() => (mangoOpen = true)}
    class="inline-flex shrink-0 items-center gap-1 rounded-lg border border-border px-2 py-1 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
  >
    <Gauge size={13} /> Configure overlay…
  </button>
{/snippet}

<Dialog
  bind:open={mangoOpen}
  title="MangoHud overlay"
  subtitle="Build the overlay, then apply it to the launch command."
  width="46rem"
>
  <MangoHud onapply={() => (mangoOpen = false)} />
</Dialog>

{#snippet row(h: Hit)}
  {#if h.kind === "wrap"}
    {@const w = h.def as WrapperDef}
    <OptionRow
      paramKey={w.key}
      enabled={app.wrap[w.key]?.enabled ?? false}
      title={h.label}
      help={w.help}
      details={w.details}
      example={w.example}
      url={w.url}
      defaultValue={w.default_value}
      valueField={w.kind === "gamescope" ? "text" : "none"}
      value={app.wrap[w.key]?.value ?? ""}
      placeholder="-W 2560 -H 1440 -f"
      requires={w.requires}
      gpu={w.gpu}
      needs={w.needs}
      dim={h.hidden}
      appliedBy={app.recipeOrigin[h.key] ?? null}
      titleRanges={h.titleRanges}
      helpRanges={h.helpRanges}
      action={w.key === "mangohud" ? configureOverlay : null}
      onToggle={() => app.toggleWrap(w.key)}
      onValue={(v) => app.setWrapValue(w.key, v)}
    />
  {:else}
    {@const e = h.def as EnvDef}
    <OptionRow
      paramKey={e.key}
      enabled={app.env[e.key]?.enabled ?? false}
      title={e.key}
      mono
      help={e.help}
      details={e.details}
      example={e.example}
      url={e.url}
      defaultValue={e.default_value}
      values={e.values}
      valueField={envValueField(e.values.length)}
      value={app.env[e.key]?.value ?? ""}
      placeholder={e.default_value || "value"}
      requires={e.requires}
      gpu={e.gpu}
      needs={e.needs}
      dim={h.hidden}
      appliedBy={app.recipeOrigin[h.key] ?? null}
      titleRanges={h.titleRanges}
      helpRanges={h.helpRanges}
      action={e.key === "MANGOHUD_CONFIG" ? configureOverlay : null}
      onToggle={() => app.toggleEnv(e.key)}
      onValue={(v) => app.setEnvValue(e.key, v)}
    />
  {/if}
{/snippet}
