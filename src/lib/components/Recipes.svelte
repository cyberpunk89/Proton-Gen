<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import { irrelevance } from "$lib/util";
  import {
    Sparkle,
    Lightning,
    Rocket,
    Monitor,
    Gauge,
    Wrench,
    ShieldCheck,
    FilmSlate,
  } from "phosphor-svelte";
  import RecipePreview from "./RecipePreview.svelte";
  import type { Recipe } from "$lib/types";

  const ICONS: Record<string, typeof Sparkle> = {
    sparkle: Sparkle,
    lightning: Lightning,
    rocket: Rocket,
    monitor: Monitor,
    gauge: Gauge,
    wrench: Wrench,
    shield: ShieldCheck,
    film: FilmSlate,
  };

  // The IPC index is the stable identity used by apply_recipe/preview_recipe —
  // it is captured here once and threaded through every filtered view. Never
  // renumber off a filtered list.
  let indexed = $derived(app.recipes.map((r, i) => ({ r, i })));

  // Collapsed by default so the recipe wall doesn't dominate the main view.
  let collapsed = $state(true);

  // Browse controls.
  type Tab = "all" | "profile" | "fix";
  let activeTab = $state<Tab>("all");
  let gpuFilter = $state<string | null>(null); // null = every GPU

  function recipeIrrelevant(r: Recipe): string | null {
    return irrelevance(app.hwCaps, r.gpu === "any" ? null : r.gpu, r.needs);
  }

  let showAll = $derived(app.store.show_irrelevant);

  /** Recipes that pass the hardware-relevance gate (or all, when "Show all"). */
  let relevant = $derived(indexed.filter((x) => showAll || !recipeIrrelevant(x.r)));
  let irrelevantTotal = $derived(indexed.filter((x) => recipeIrrelevant(x.r)).length);

  /** GPU vendors present among the relevant recipes, for the filter chips. */
  let gpuOptions = $derived([
    ...new Set(
      relevant.map((x) => x.r.gpu).filter((g): g is string => !!g && g !== "any"),
    ),
  ]);

  const GPU_LABELS: Record<string, string> = { nvidia: "NVIDIA", amd: "AMD", intel: "Intel" };

  /** A recipe matches the GPU chip if it targets that vendor or is universal. */
  function matchesGpu(r: Recipe): boolean {
    if (gpuFilter === null) return true;
    return r.gpu === gpuFilter || r.gpu === "any" || r.gpu === null;
  }

  let shown = $derived(
    relevant.filter(
      (x) => (activeTab === "all" || x.r.kind === activeTab) && matchesGpu(x.r),
    ),
  );

  let profiles = $derived(shown.filter((x) => x.r.kind === "profile"));
  let fixes = $derived(shown.filter((x) => x.r.kind === "fix"));

  // Tab counts respect the GPU chip but not the tab itself.
  let gpuScoped = $derived(relevant.filter((x) => matchesGpu(x.r)));
  let counts = $derived({
    all: gpuScoped.length,
    profile: gpuScoped.filter((x) => x.r.kind === "profile").length,
    fix: gpuScoped.filter((x) => x.r.kind === "fix").length,
  });

  const TABS: { id: Tab; label: string }[] = [
    { id: "all", label: "All" },
    { id: "profile", label: "Profiles" },
    { id: "fix", label: "Troubleshooter" },
  ];

  async function apply(i: number, name: string) {
    await app.applyRecipe(i);
    toast.success(`Applied: ${name}`);
  }
</script>

<section class="card p-4">
  <button
    class="flex w-full items-center gap-2"
    onclick={() => (collapsed = !collapsed)}
    aria-expanded={!collapsed}
    aria-controls="recipes-body"
  >
    <Sparkle size={18} weight="fill" class="text-accent" />
    <h2 class="text-sm font-medium tracking-wide text-text">Recipes</h2>
    <span class="text-xs text-muted">one-click tuning, merges onto your selection</span>
    <span class="ml-auto text-xs text-muted">{collapsed ? "Show" : "Hide"}</span>
  </button>

  {#if !collapsed}
    <div id="recipes-body" class="mt-4 space-y-4">
      <!-- Type tabs -->
      <div class="flex gap-1 rounded-lg bg-mantle p-0.5 text-xs">
        {#each TABS as t (t.id)}
          <button
            onclick={() => (activeTab = t.id)}
            class="flex-1 rounded-md px-2 py-1 font-medium transition"
            class:bg-surface-2={activeTab === t.id}
            class:text-text={activeTab === t.id}
            class:text-muted={activeTab !== t.id}
          >
            {t.label}
            <span class="opacity-60">({counts[t.id]})</span>
          </button>
        {/each}
      </div>

      <!-- GPU filter chips (only when there's more than one vendor to pick from) -->
      {#if gpuOptions.length > 1}
        <div class="flex flex-wrap items-center gap-1.5">
          <button
            onclick={() => (gpuFilter = null)}
            class="rounded-full px-2.5 py-0.5 text-[11px] transition"
            class:bg-accent={gpuFilter === null}
            class:text-on-accent={gpuFilter === null}
            class:bg-surface-2={gpuFilter !== null}
            class:text-muted={gpuFilter !== null}>All GPUs</button
          >
          {#each gpuOptions as g (g)}
            <button
              onclick={() => (gpuFilter = g)}
              class="rounded-full px-2.5 py-0.5 text-[11px] transition"
              class:bg-accent={gpuFilter === g}
              class:text-on-accent={gpuFilter === g}
              class:bg-surface-2={gpuFilter !== g}
              class:text-muted={gpuFilter !== g}>{GPU_LABELS[g] ?? g}</button
            >
          {/each}
        </div>
      {/if}

      {#if irrelevantTotal > 0}
        <div class="flex items-center gap-2 text-xs text-muted">
          {#if showAll}
            <span>Showing all recipes, including unsupported ones.</span>
            <button
              onclick={() => app.setShowIrrelevant(false)}
              class="font-medium text-accent hover:underline">Hide unsupported</button
            >
          {:else}
            <span>{irrelevantTotal} hidden for your hardware.</span>
            <button
              onclick={() => app.setShowIrrelevant(true)}
              class="font-medium text-accent hover:underline">Show all</button
            >
          {/if}
        </div>
      {/if}

      {#if !shown.length}
        <p class="py-8 text-center text-sm text-muted">No recipes match this filter.</p>
      {:else if activeTab === "all"}
        {@render group("Profiles", profiles)}
        {@render group("Troubleshooter", fixes)}
      {:else}
        {@render group(null, shown)}
      {/if}
    </div>
  {/if}
</section>

{#snippet group(title: string | null, items: { r: Recipe; i: number }[])}
  {#if items.length}
    <div>
      {#if title}
        <h3 class="mb-2 text-[11px] font-medium uppercase tracking-wider text-muted">
          {title}
        </h3>
      {/if}
      <div class="grid grid-cols-1 gap-2.5 sm:grid-cols-2 xl:grid-cols-3">
        {#each items as { r, i } (r.name)}
          {@const Icon = ICONS[r.icon ?? ""] ?? Sparkle}
          {@const dim = recipeIrrelevant(r)}
          {@const accent = r.accent ?? "var(--accent)"}
          <div
            class="group flex flex-col rounded-xl border border-border/70 bg-surface-solid/40 p-3 transition hover:border-[color:var(--rc)]"
            style="--rc: {accent}; {dim ? 'opacity:.6' : ''}"
          >
            <div class="flex items-start gap-2.5">
              <span
                class="grid size-8 shrink-0 place-items-center rounded-lg"
                style="background: color-mix(in srgb, {accent} 18%, transparent); color: {accent}"
              >
                <Icon size={17} weight="fill" />
              </span>
              <div class="min-w-0 flex-1">
                <p class="truncate text-sm font-medium text-text">{r.name}</p>
                {#if r.symptom}
                  <p class="text-xs italic text-muted">{r.symptom}</p>
                {/if}
              </div>
            </div>

            {#if r.description}
              <p class="mt-2 flex-1 text-xs leading-snug text-subtext">{r.description}</p>
            {/if}

            <div class="mt-2.5 flex items-center gap-1.5">
              {#each r.tags as t (t)}
                <span class="rounded-full bg-surface-2 px-2 py-0.5 text-[10px] text-muted"
                  >{t}</span
                >
              {/each}
              {#if dim}
                <span class="text-[10px] text-peach">{dim}</span>
              {/if}
              <RecipePreview index={i} {accent} />
              <button
                onclick={() => apply(i, r.name)}
                class="ml-auto rounded-lg px-2.5 py-1 text-xs font-medium transition active:scale-95"
                style="background: color-mix(in srgb, {accent} 20%, transparent); color: {accent}"
              >
                Apply
              </button>
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}
{/snippet}
