<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import { irrelevance } from "$lib/util";
  import { isAdvanced } from "$lib/types";
  import type { Recipe } from "$lib/types";
  import { fuzzy } from "$lib/fuzzy";
  import OptionRow from "./OptionRow.svelte";
  import Recipes from "./Recipes.svelte";
  import GameRuntimePanel from "./GameRuntimePanel.svelte";
  import ActiveOptions from "./ActiveOptions.svelte";
  import Dialog from "./Dialog.svelte";
  import MangoHud from "./MangoHud.svelte";
  import OptiScaler from "./OptiScaler.svelte";
  import { MagnifyingGlass, Faders, Gauge, Sparkle } from "phosphor-svelte";
  import type { EnvDef, WrapperDef } from "$lib/types";

  /**
   * The overlay builder. Lives here rather than inside the row so it survives a
   * section change (or the row scrolling out of a search result) while open,
   * and so both of its entry points share one instance.
   */
  let mangoOpen = $state(false);

  /** The OptiScaler builder — same rationale as the overlay builder above. */
  let optiOpen = $state(false);

  let showAll = $derived(app.store.show_irrelevant);
  let showAdvanced = $derived(app.store.show_advanced);
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
    /**
     * Tagged `tier = "advanced"`; filtered out unless "Show advanced" is on.
     *
     * Kept separate from `hidden` rather than folded into it: the two answer
     * different questions ("does this apply to my machine?" vs "do I want to see
     * the long tail?"), have separate toggles, and share the summary row below —
     * one flag would make its counts lie.
     */
    advanced: boolean;
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
          advanced: isAdvanced(e),
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
          advanced: isAdvanced(e),
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
          advanced: isAdvanced(w),
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
          advanced: isAdvanced(w),
        });
      }
    }
    return out;
  });

  /**
   * Recipes matching the query, so a search for "hdr" or "gamemode" also turns
   * up the one-click bundles, not only the raw env vars. Kept separate from the
   * env/wrapper `hits` because a recipe applies as a unit rather than toggling a
   * single key — and crucially we keep the *original* index into `app.recipes`,
   * which is the stable handle `applyRecipe` needs (never a filtered index).
   */
  interface RecipeHit {
    index: number;
    recipe: Recipe;
    score: number;
  }
  let recipeHits = $derived.by((): RecipeHit[] => {
    if (!searching) return [];
    const query = q.trim();
    const out: RecipeHit[] = [];
    app.recipes.forEach((r, index) => {
      // Respect the hardware filter, matching the parameter rows above.
      if (!showAll && irrelevance(app.hwCaps, r.gpu, r.needs)) return;
      const t = fuzzy(r.name, query, r.tags);
      const h = fuzzy(r.description, query, r.symptom ? [r.symptom] : []);
      if (!t && !h) return;
      out.push({ index, recipe: r, score: (t?.score ?? 0) * 2 + (h?.score ?? 0) });
    });
    return out.sort((a, b) => b.score - a.score);
  });

  function applyRecipe(index: number, name: string) {
    app.applyRecipe(index);
    toast.success(`Applied “${name}”`);
  }

  let visible = $derived(
    hits.filter((h) => (showAll || !h.hidden) && (showAdvanced || !h.advanced)),
  );
  let hiddenCount = $derived(hits.filter((h) => h.hidden).length);
  // Counted only among rows the hardware filter would have shown anyway, so the
  // two numbers in the summary row never double-count the same option.
  let advancedCount = $derived(
    hits.filter((h) => h.advanced && (showAll || !h.hidden)).length,
  );
  let anyShown = $derived(visible.length > 0 || recipeHits.length > 0);

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
    }${
      recipeHits.length ? ` · ${recipeHits.length} recipe${recipeHits.length === 1 ? "" : "s"}` : ""
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

    <!-- Two independent filters, one line. Each half only appears when it has
         something to say, so the common case is a single short sentence. -->
    {#if hiddenCount > 0 || advancedCount > 0 || showAdvanced}
      <div class="mb-3 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted">
        {#if hiddenCount > 0}
          <span class="inline-flex items-center gap-2">
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
          </span>
        {/if}
        {#if advancedCount > 0}
          <span class="inline-flex items-center gap-2">
            {#if showAdvanced}
              <span>{advancedCount} advanced shown.</span>
              <button
                onclick={() => app.setShowAdvanced(false)}
                class="font-medium text-accent hover:underline">Hide advanced</button
              >
            {:else}
              <span>{advancedCount} advanced hidden.</span>
              <button
                onclick={() => app.setShowAdvanced(true)}
                class="font-medium text-accent hover:underline">Show advanced</button
              >
            {/if}
          </span>
        {:else if showAdvanced}
          <!-- Nothing advanced in this section, but the toggle is on globally —
               say so, or "Show advanced" looks like it did nothing. -->
          <span>Advanced options are shown; this section has none.</span>
        {/if}
      </div>
    {/if}

    {#if !anyShown}
      <div class="flex flex-col items-center gap-2 py-12 text-center">
        <MagnifyingGlass size={26} class="text-muted" />
        <!-- Name the filter that actually emptied this, rather than blaming
             hardware for what the advanced tier did. Whole sections (VKD3D,
             Wine / Overrides) are advanced end to end, so the wrong reason here
             would send you looking for a hardware problem you don't have. The
             matching "Show advanced" link is in the summary row above. -->
        <p class="text-sm text-muted">
          {searching
            ? `No parameters match “${q.trim()}”.`
            : advancedCount > 0
              ? "Everything here is an advanced option."
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
        {#if recipeHits.length}
          <div>
            <p class="mb-1 px-3 text-[11px] font-medium uppercase tracking-wider text-muted">
              Recipes
            </p>
            <div class="space-y-1">
              {#each recipeHits as rh (rh.recipe.name)}
                {@render recipeRow(rh.index, rh.recipe)}
              {/each}
            </div>
          </div>
        {/if}
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

<!-- Wired to both the PROTON_USE_OPTISCALER toggle and the
     PROTON_OPTISCALER_CONFIG row: either is where someone looks for it. -->
{#snippet configureOptiScaler()}
  <button
    type="button"
    onclick={() => (optiOpen = true)}
    class="inline-flex shrink-0 items-center gap-1 rounded-lg border border-border px-2 py-1 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
  >
    <Sparkle size={13} /> Configure OptiScaler…
  </button>
{/snippet}

<Dialog
  bind:open={optiOpen}
  title="OptiScaler"
  subtitle="Compose the upscaler config, then apply it to the launch command."
  width="46rem"
>
  <OptiScaler onapply={() => (optiOpen = false)} />
</Dialog>

{#snippet recipeRow(index: number, r: Recipe)}
  <div class="flex items-center gap-3 rounded-lg px-3 py-2 transition hover:bg-surface-2/50">
    <span class="min-w-0 flex-1">
      <span class="flex items-center gap-1.5 text-sm text-text">
        <Sparkle size={13} class="shrink-0 text-accent" />
        <span class="truncate">{r.name}</span>
        <span
          class="shrink-0 rounded px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-muted"
          style="background: color-mix(in srgb, var(--surface-2) 70%, transparent)"
          >{r.kind === "fix" ? "fix" : "profile"}</span
        >
      </span>
      <span class="mt-0.5 block truncate text-xs text-muted">{r.description}</span>
    </span>
    <button
      onclick={() => applyRecipe(index, r.name)}
      class="shrink-0 rounded-lg border border-border px-2.5 py-1 text-xs font-medium text-subtext transition hover:border-accent/50 hover:text-text"
    >
      Apply
    </button>
  </div>
{/snippet}

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
      action={e.key === "MANGOHUD_CONFIG"
        ? configureOverlay
        : e.key === "PROTON_OPTISCALER_CONFIG" || e.key === "PROTON_USE_OPTISCALER"
          ? configureOptiScaler
          : null}
      onToggle={() => app.toggleEnv(e.key)}
      onValue={(v) => app.setEnvValue(e.key, v)}
    />
  {/if}
{/snippet}
