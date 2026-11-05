<script lang="ts">
  import { app, NON_CATALOG_SECTIONS } from "$lib/state.svelte";
  import { focusTarget } from "$lib/actions";
  import { prefersReducedMotion } from "$lib/motion.svelte";
  import UiModeToggle from "./UiModeToggle.svelte";
  import type { Severity } from "$lib/types";
  import { toast } from "$lib/toast.svelte";
  import { irrelevance } from "$lib/util";
  import { isAdvanced } from "$lib/types";
  import type { Recipe } from "$lib/types";
  import { fuzzy } from "$lib/fuzzy";
  import OptionRow from "./OptionRow.svelte";
  import Recipes from "./Recipes.svelte";
  import GameRuntimePanel from "./GameRuntimePanel.svelte";
  import {
    MagnifyingGlass,
    Faders,
    FilmStrip,
    Gauge,
    Monitor,
    Sparkle,
    X,
    Cpu,
    WarningOctagon,
    Warning,
    Info,
  } from "phosphor-svelte";
  import type { EnvDef, WrapperDef } from "$lib/types";

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

  /** The rows that pick lsfg-vk's mode, each with a "Configure…" button. */
  const LSFG_MODE_KEYS = new Set(["LSFGVK_PROFILE", "LSFGVK_ENV", "DISABLE_LSFGVK"]);

  /** A row's suggested values. LSFGVK_PROFILE offers the profile names from
   *  the user's own conf.toml instead of the catalog's placeholder. */
  function envValues(e: EnvDef): string[] {
    if (e.key === "LSFGVK_PROFILE") {
      const names = app.lsfgStatus?.profiles.map((p) => p.name) ?? [];
      if (names.length) return names;
    }
    return e.values;
  }

  /** Profile names are a pick list however many there are — one profile still
   *  needs a field, which `envValueField(1)` would drop. */
  function envField(e: EnvDef): "none" | "text" | "segmented" | "select" {
    const n = envValues(e).length;
    return e.key === "LSFGVK_PROFILE" && n > 0 ? "select" : envValueField(n);
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

  function applyRecipe(index: number) {
    void app.applyRecipe(index);
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

  let resultSummary = $derived(
    `${visible.length} match${visible.length === 1 ? "" : "es"} in ${groups.length} categor${
      groups.length === 1 ? "y" : "ies"
    }${
      recipeHits.length ? ` · ${recipeHits.length} recipe${recipeHits.length === 1 ? "" : "s"}` : ""
    }`,
  );

  // ------------------------- the one scrolling catalog -------------------------

  /** Which view the panel shows: the catalog, or one of the non-catalog tabs. */
  let tab = $derived(NON_CATALOG_SECTIONS.has(app.activeSection) ? app.activeSection : "catalog");

  /** Every section of the list, in order, with its visible rows. Wrappers first,
   *  as the rail had it. A section the advanced tier emptied keeps its header, so
   *  "Show advanced" stays reachable by browsing. */
  let sections = $derived.by(() => {
    const names = ["Wrappers", ...app.visibleCategories];
    const by = new Map<string, Hit[]>(names.map((n) => [n, []]));
    for (const h of visible) by.get(h.group)?.push(h);
    const advIn = new Map<string, number>();
    if (!showAdvanced) {
      for (const h of hits) {
        if (h.advanced && (showAll || !h.hidden)) advIn.set(h.group, (advIn.get(h.group) ?? 0) + 1);
      }
    }
    return names
      .map((name) => ({
        name,
        rows: by.get(name) ?? [],
        advancedHidden: advIn.get(name) ?? 0,
        on:
          name === "Wrappers"
            ? app.catalog.wrappers.filter((w) => app.wrap[w.key]?.enabled).length
            : app.enabledCountInCategory(name),
      }))
      .filter((sec) => sec.rows.length || sec.advancedHidden);
  });

  /** Recommended-for-GPU keys still to apply, per category. */
  let recommendedIn = $derived.by(() => {
    const m = new Map<string, number>();
    for (const e of app.catalog.envs) {
      if (app.recommendedEnvKeys.includes(e.key)) m.set(e.category, (m.get(e.category) ?? 0) + 1);
    }
    return m;
  });

  let panel = $state<HTMLElement | null>(null);
  let root = $derived(panel?.closest<HTMLElement>("[data-scroll-root]") ?? null);
  let toolbar = $state<HTMLElement | null>(null);
  let toolbarH = $state(0);
  /** The section at the top of the viewport — the chip it lights up. */
  let spy = $state<string | null>(null);

  function sectionEl(name: string): HTMLElement | null {
    return panel?.querySelector<HTMLElement>(`[data-section="${CSS.escape(name)}"]`) ?? null;
  }

  /** Scroll-spy: the last section whose header has reached the toolbar. */
  function measure() {
    if (!root || !panel || tab !== "catalog" || searching) return;
    const line = root.getBoundingClientRect().top + (toolbar?.offsetHeight ?? toolbarH) + 8;
    let current: string | null = sections[0]?.name ?? null;
    for (const el of panel.querySelectorAll<HTMLElement>("[data-section]")) {
      if (el.getBoundingClientRect().top <= line) current = el.dataset.section ?? current;
      else break;
    }
    if (current !== spy) spy = current;
    // Remembered so leaving for Recipes and coming back lands here again.
    if (current && app.activeSection !== current) app.activeSection = current;
  }

  $effect(() => {
    const r = root;
    if (!r) return;
    let raf = 0;
    const onScroll = () => {
      if (raf) return;
      raf = requestAnimationFrame(() => {
        raf = 0;
        measure();
      });
    };
    r.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      r.removeEventListener("scroll", onScroll);
      if (raf) cancelAnimationFrame(raf);
    };
  });

  // A chip click, palette jump or the Parameters tab: scroll to the section.
  $effect(() => {
    const jump = app.sectionJump;
    if (!jump || !root || tab !== "catalog" || searching) return;
    const el = sectionEl(jump.section);
    if (!el) return;
    // Measured now, not from `toolbarH`: switching tabs adds the chip row in
    // this same update, before bind:clientHeight has reported it.
    const offset = toolbar?.offsetHeight ?? toolbarH;
    const top =
      el.getBoundingClientRect().top - root.getBoundingClientRect().top + root.scrollTop - offset;
    root.scrollTo({ top: Math.max(0, top), behavior: prefersReducedMotion() ? "auto" : "smooth" });
    spy = jump.section;
    app.sectionJump = null;
  });

  function goCatalog() {
    app.setSection(spy ?? "Wrappers");
  }

  // ------------------------------- inline lint -------------------------------

  let counts = $derived({
    error: app.notices.filter((n) => n.severity === "error").length,
    warning: app.notices.filter((n) => n.severity === "warning").length,
    info: app.notices.filter((n) => n.severity === "info").length,
  });
  let worst = $derived<Severity | null>(
    counts.error ? "error" : counts.warning ? "warning" : counts.info ? "info" : null,
  );
  const SEV_COLOUR: Record<Severity, string> = {
    error: "var(--red)",
    warning: "var(--peach)",
    info: "var(--blue)",
  };
  const SEV_ICON: Record<Severity, typeof Info> = { error: WarningOctagon, warning: Warning, info: Info };

  /**
   * Escape clears the search rather than leaving the builder. Handled here with
   * stopPropagation, not in the global binding table: the global Escape means
   * "back to the library", and it can't know this field has text worth
   * clearing first. Only swallowed when there's something to clear.
   */
  function onSearchKeydown(e: KeyboardEvent) {
    if (e.key !== "Escape" || !app.paramQuery) return;
    e.stopPropagation();
    e.preventDefault();
    app.paramQuery = "";
  }

  function applyRecommended() {
    app.applyRecommendedForGpu();
    toast.success("Applied GPU-recommended defaults");
  }
</script>

<div bind:this={panel} class="flex flex-col gap-3">
  <!--
    The toolbar sticks to the top of the scroll region: view tabs, the parameter
    search, the lint count, and — in the catalog — one chip per category that
    jumps there and lights up as you scroll past it. Bleeds into the column's
    padding so rows scrolling under it don't peek out at the sides.
  -->
  <div
    bind:this={toolbar}
    bind:clientHeight={toolbarH}
    class="sticky top-0 z-20 -mx-5 space-y-2.5 px-5 pb-2.5 pt-2"
    style="background: var(--bg)"
  >
    <div class="flex flex-wrap items-center gap-2">
      <UiModeToggle />
      <div role="tablist" aria-label="Builder views" class="inline-flex rounded-lg border border-border bg-surface-2/50 p-0.5">
        {@render tabButton("catalog", "Parameters", Faders, goCatalog)}
        {@render tabButton("recipes", "Recipes", Sparkle, () => app.setSection("recipes"))}
        {@render tabButton("game", "Game & runtime", Cpu, () => app.setSection("game"))}
      </div>

      <div class="relative min-w-48 flex-1">
        <MagnifyingGlass
          size={14}
          class="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"
        />
        <input
          use:focusTarget={"param-search"}
          bind:value={app.paramQuery}
          onkeydown={onSearchKeydown}
          aria-label="Search parameters"
          placeholder="Search parameters and recipes…  /"
          class="h-8 w-full rounded-lg border border-border bg-surface-2 pl-8 pr-7 text-xs text-text outline-none focus:border-accent"
        />
        {#if app.paramQuery}
          <button
            onclick={() => (app.paramQuery = "")}
            class="absolute right-2 top-1/2 -translate-y-1/2 text-muted hover:text-text"
            aria-label="Clear search"
          >
            <X size={13} />
          </button>
        {/if}
      </div>

      {#if worst}
        {@const Icon = SEV_ICON[worst]}
        <!-- Lint lives on the rows now; this is the count, and the full list
             (including notices with no row to sit on) one click away. -->
        <button
          onclick={() => app.openDock("problems")}
          title="Open Problems"
          class="inline-flex h-8 items-center gap-1.5 rounded-lg border px-2.5 text-xs font-medium transition hover:brightness-110"
          style="border-color: color-mix(in srgb, {SEV_COLOUR[worst]} 40%, transparent);
                 background: color-mix(in srgb, {SEV_COLOUR[worst]} 10%, transparent);
                 color: {SEV_COLOUR[worst]}"
        >
          <Icon size={14} weight="fill" />
          {[
            counts.error && `${counts.error} problem${counts.error > 1 ? "s" : ""}`,
            counts.warning && `${counts.warning} warning${counts.warning > 1 ? "s" : ""}`,
            counts.info && `${counts.info} hint${counts.info > 1 ? "s" : ""}`,
          ]
            .filter(Boolean)
            .join(" · ")}
        </button>
      {/if}
    </div>

    {#if tab === "catalog" && !searching}
      <nav aria-label="Parameter categories" class="flex flex-wrap gap-1">
        {#each sections as sec (sec.name)}
          {@const on = spy === sec.name}
          <button
            onclick={() => app.setSection(sec.name)}
            aria-current={on ? "location" : undefined}
            class="inline-flex items-center gap-1 rounded-full border px-2.5 py-0.5 text-[11px] transition {on
              ? 'border-accent/50'
              : 'border-border/70 text-subtext hover:border-accent/40 hover:text-text'}"
            style={on ? "background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent)" : ""}
          >
            {sec.name}
            {#if sec.on}
              <span
                class="rounded-full px-1 text-[10px]"
                style="background: color-mix(in srgb, var(--accent) 20%, transparent); color: var(--accent)"
                >{sec.on}</span
              >
            {/if}
          </button>
        {/each}
      </nav>
    {/if}

    {#if (tab === "catalog" || searching) && (hiddenCount > 0 || advancedCount > 0 || showAdvanced)}
      {@render filterSummary()}
    {/if}
  </div>


  {#if searching}
    {@render searchResults()}
  {:else if tab === "recipes"}
    <Recipes />
  {:else if tab === "game"}
    <GameRuntimePanel />
  {:else if !sections.length}
    <div class="card flex flex-col items-center gap-2 py-12 text-center">
      <MagnifyingGlass size={26} class="text-muted" />
      <p class="text-sm text-muted">Nothing to show here for your hardware.</p>
    </div>
  {:else}
    {#each sections as sec (sec.name)}
      <section data-section={sec.name} aria-label={sec.name} class="card p-2">
        <!-- Sticks just under the toolbar while its rows scroll past. -->
        <div
          class="sticky z-10 -mx-2 -mt-2 mb-1 flex items-center gap-2 rounded-t-[var(--radius-card)] px-4 py-2"
          style="top: {toolbarH}px; background: var(--surface-solid)"
        >
          <h2 class="text-sm font-medium tracking-wide text-text">{sec.name}</h2>
          <span class="text-[11px] text-muted"
            >{sec.rows.length}{sec.on ? ` · ${sec.on} on` : ""}</span
          >
          {#if recommendedIn.get(sec.name)}
            <button
              onclick={applyRecommended}
              class="ml-auto inline-flex items-center gap-1.5 rounded-lg border border-accent/40 bg-accent/5 px-2.5 py-0.5 text-xs font-medium text-accent transition hover:bg-accent/10"
            >
              <Sparkle size={12} weight="fill" /> Recommended for your GPU
            </button>
          {/if}
        </div>
        <div class="space-y-0.5">
          {#each sec.rows as h (h.kind + h.key)}
            {@render row(h)}
          {/each}
        </div>
        {#if sec.advancedHidden}
          <p class="px-3 py-2 text-xs text-muted">
            {sec.advancedHidden} advanced option{sec.advancedHidden === 1 ? "" : "s"} hidden here.
            <button onclick={() => app.setShowAdvanced(true)} class="font-medium text-accent hover:underline"
              >Show advanced</button
            >
          </p>
        {/if}
      </section>
    {/each}
  {/if}
</div>

{#snippet tabButton(
  id: string,
  label: string,
  Icon: typeof Faders,
  go: () => void,
  count = 0,
)}
  {@const active = !searching && tab === id}
  <button
    role="tab"
    aria-selected={active}
    onclick={go}
    class="inline-flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs transition {active
      ? 'font-medium'
      : 'text-muted hover:text-text'}"
    style={active ? "background: var(--accent); color: var(--on-accent)" : ""}
  >
    <Icon size={13} weight={active ? "fill" : "regular"} />
    {label}
    {#if count > 0}
      <span
        class="rounded-full px-1 text-[10px]"
        style={active
          ? "background: color-mix(in srgb, var(--on-accent) 20%, transparent)"
          : "background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent)"}
        >{count}</span
      >
    {/if}
  </button>
{/snippet}

<!-- Two independent filters, one line. Each half only appears when it has
     something to say, so the common case is a single short sentence. -->
{#snippet filterSummary()}
  <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted">
    {#if searching}
      <span class="text-subtext">{resultSummary}</span>
    {/if}
    {#if hiddenCount > 0}
      <span class="inline-flex items-center gap-1.5">
        {#if showAll}
          <span>Showing options your hardware can't use.</span>
          <button onclick={() => app.setShowIrrelevant(false)} class="font-medium text-accent hover:underline"
            >Hide them</button
          >
        {:else}
          <span>{hiddenCount} hidden for your hardware.</span>
          <button onclick={() => app.setShowIrrelevant(true)} class="font-medium text-accent hover:underline"
            >Show all</button
          >
        {/if}
      </span>
    {/if}
    {#if advancedCount > 0 || showAdvanced}
      <span class="inline-flex items-center gap-1.5">
        {#if showAdvanced}
          <span>Advanced options shown.</span>
          <button onclick={() => app.setShowAdvanced(false)} class="font-medium text-accent hover:underline"
            >Hide advanced</button
          >
        {:else}
          <span>{advancedCount} advanced hidden.</span>
          <button onclick={() => app.setShowAdvanced(true)} class="font-medium text-accent hover:underline"
            >Show advanced</button
          >
        {/if}
      </span>
    {/if}
  </div>
{/snippet}

{#snippet searchResults()}
  <section class="card p-4">
    {#if !anyShown}
      <div class="flex flex-col items-center gap-2 py-12 text-center">
        <MagnifyingGlass size={26} class="text-muted" />
        <!-- Name the filter that emptied this, rather than blaming hardware
             for what the advanced tier did. -->
        <p class="text-sm text-muted">
          No parameters match “{q.trim()}”{advancedCount > 0 && !showAdvanced
            ? " — some advanced ones might"
            : ""}.
        </p>
        <button onclick={() => (app.paramQuery = "")} class="text-xs text-accent hover:underline"
          >Clear search</button
        >
      </div>
    {:else}
      <!-- Grouped so a 40-result query reads as a map of the catalog rather than
           a flat wall. Groups ordered by their best member, items by score. -->
      <div class="space-y-4">
        {#if recipeHits.length}
          <div>
            <p class="mb-1 px-3 text-[11px] font-medium uppercase tracking-wider text-muted">Recipes</p>
            <div class="space-y-1">
              {#each recipeHits as rh (rh.recipe.name)}
                {@render recipeRow(rh.index, rh.recipe)}
              {/each}
            </div>
          </div>
        {/if}
        {#each groups as g (g.name)}
          <div>
            <p class="mb-1 px-3 text-[11px] font-medium uppercase tracking-wider text-muted">{g.name}</p>
            <div class="space-y-0.5">
              {#each g.items as h (h.kind + h.key)}
                {@render row(h)}
              {/each}
            </div>
          </div>
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
    onclick={() => (app.mangoBuilderOpen = true)}
    class="inline-flex shrink-0 items-center gap-1 rounded-lg border border-border px-2 py-1 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
  >
    <Gauge size={13} /> Configure overlay…
  </button>
{/snippet}

<!-- Wired to both the PROTON_USE_OPTISCALER toggle and the
     PROTON_OPTISCALER_CONFIG row: either is where someone looks for it. -->
{#snippet configureOptiScaler()}
  <button
    type="button"
    onclick={() => (app.optiBuilderOpen = true)}
    class="inline-flex shrink-0 items-center gap-1 rounded-lg border border-border px-2 py-1 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
  >
    <Sparkle size={13} /> Configure OptiScaler…
  </button>
{/snippet}

<!-- Wired to both the ENABLE_VKBASALT toggle and the VKBASALT_CONFIG_FILE
     row: either is where someone looks for it. -->
{#snippet configureVkBasalt()}
  <button
    type="button"
    onclick={() => (app.vkBuilderOpen = true)}
    class="inline-flex shrink-0 items-center gap-1 rounded-lg border border-border px-2 py-1 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
  >
    <Sparkle size={13} /> Configure vkBasalt…
  </button>
{/snippet}

<!-- Wired to the rows that pick a mode (profile / per-game / off): the
     builder owns all three and keeps them from contradicting each other. -->
<!-- The gamescope row: its free-text args field stays for hand edits; the
     builder covers the common flags and keeps the rest. -->
{#snippet configureGamescope()}
  <button
    type="button"
    onclick={() => (app.gamescopeBuilderOpen = true)}
    class="inline-flex shrink-0 items-center gap-1 rounded-lg border border-border px-2 py-1 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
  >
    <Monitor size={13} /> Configure gamescope…
  </button>
{/snippet}

{#snippet configureDllOverrides()}
  <button
    type="button"
    onclick={() => (app.dllBuilderOpen = true)}
    class="inline-flex shrink-0 items-center gap-1 rounded-lg border border-border px-2 py-1 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
  >
    <Faders size={13} /> Edit overrides…
  </button>
{/snippet}

{#snippet configureLsfg()}
  <button
    type="button"
    onclick={() => (app.lsfgBuilderOpen = true)}
    class="inline-flex shrink-0 items-center gap-1 rounded-lg border border-border px-2 py-1 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
  >
    <FilmStrip size={13} /> Configure lsfg-vk…
  </button>
{/snippet}

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
      onclick={() => applyRecipe(index)}
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
      wrapper
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
      pkg={w.pkg}
      gpu={w.gpu}
      needs={w.needs}
      dim={h.hidden}
      appliedBy={app.recipeOrigin[h.key] ?? null}
      notices={app.noticesByKey.get(h.key) ?? []}
      titleRanges={h.titleRanges}
      helpRanges={h.helpRanges}
      action={w.key === "mangohud"
        ? configureOverlay
        : w.kind === "gamescope"
          ? configureGamescope
          : null}
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
      values={envValues(e)}
      valueField={envField(e)}
      value={app.env[e.key]?.value ?? ""}
      placeholder={e.default_value || "value"}
      requires={e.requires}
      pkg={e.pkg}
      gpu={e.gpu}
      needs={e.needs}
      dim={h.hidden}
      appliedBy={app.recipeOrigin[h.key] ?? null}
      notices={app.noticesByKey.get(h.key) ?? []}
      titleRanges={h.titleRanges}
      helpRanges={h.helpRanges}
      action={e.key === "MANGOHUD_CONFIG"
        ? configureOverlay
        : e.key === "PROTON_OPTISCALER_CONFIG" || e.key === "PROTON_USE_OPTISCALER"
          ? configureOptiScaler
          : e.key === "ENABLE_VKBASALT" || e.key === "VKBASALT_CONFIG_FILE"
            ? configureVkBasalt
            : LSFG_MODE_KEYS.has(e.key)
              ? configureLsfg
              : e.key === "WINEDLLOVERRIDES"
                ? configureDllOverrides
                : null}
      onToggle={() => app.toggleEnv(e.key)}
      onValue={(v) => app.setEnvValue(e.key, v)}
    />
  {/if}
{/snippet}
