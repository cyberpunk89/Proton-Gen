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

  let indexed = $derived(app.recipes.map((r, i) => ({ r, i })));
  let profiles = $derived(indexed.filter((x) => x.r.kind === "profile"));
  let fixes = $derived(indexed.filter((x) => x.r.kind === "fix"));

  let collapsed = $state(false);

  function recipeIrrelevant(r: Recipe): string | null {
    return irrelevance(app.hwCaps, r.gpu === "any" ? null : r.gpu, r.needs);
  }

  let showAll = $derived(app.store.show_irrelevant);
  let irrelevantTotal = $derived(
    indexed.filter((x) => recipeIrrelevant(x.r)).length,
  );

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

  {#if !collapsed && irrelevantTotal > 0}
    <div class="mt-3 flex items-center gap-2 text-xs text-muted">
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

  {#if !collapsed}
    <div id="recipes-body" class="mt-4 space-y-4">
      {@render group("Profiles", profiles)}
      {@render group("Troubleshooter", fixes)}
    </div>
  {/if}
</section>

{#snippet group(title: string, items: { r: Recipe; i: number }[])}
  {@const vis = showAll ? items : items.filter((x) => !recipeIrrelevant(x.r))}
  {#if vis.length}
    <div>
      <h3 class="mb-2 text-[11px] font-medium uppercase tracking-wider text-muted">
        {title}
      </h3>
      <div class="grid grid-cols-1 gap-2.5 sm:grid-cols-2 xl:grid-cols-3">
        {#each vis as { r, i } (r.name)}
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
