<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { irrelevance } from "$lib/util";
  import { fuzzy } from "$lib/fuzzy";
  import Notices from "./Notices.svelte";
  import RecipePreview from "./RecipePreview.svelte";
  import Lazy from "./Lazy.svelte";
  import { lazy } from "$lib/lazy";
  import { Wrench, MagnifyingGlass, Warning } from "phosphor-svelte";

  /**
   * The dock's Problems tab: everything about "why doesn't this work" in one
   * place — lint on the current command, the symptom → fix recipes, and (with
   * the AI coach on) the free-text troubleshooter. These were a banner stack,
   * a recipes sub-tab and a header dialog.
   */
  let q = $state("");

  /** Fix recipes for this machine, best symptom match first when searching.
   *  Keeps the original index: it's `applyRecipe`'s handle. */
  let fixes = $derived.by(() => {
    const query = q.trim();
    const out: { index: number; score: number }[] = [];
    app.recipes.forEach((r, index) => {
      if (r.kind !== "fix") return;
      if (!app.store.show_irrelevant && irrelevance(app.hwCaps, r.gpu, r.needs)) return;
      if (!query) return out.push({ index, score: 0 });
      const hit = fuzzy(`${r.symptom ?? ""} ${r.name}`, query, r.tags);
      const h2 = fuzzy(r.description, query);
      if (hit || h2) out.push({ index, score: (hit?.score ?? 0) * 2 + (h2?.score ?? 0) });
    });
    return query ? out.sort((a, b) => b.score - a.score) : out;
  });
</script>

<div class="grid gap-5 lg:grid-cols-2">
  <div class="min-w-0 space-y-4">
    <section>
      <p class="mb-2 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted">
        <Warning size={13} /> In this command
      </p>
      <Notices bare />
    </section>

    {#if app.store.llm_enabled}
      <section class="border-t border-border/60 pt-4">
        <Lazy load={lazy.troubleshooter}>
          {#snippet children(C)}<C />{/snippet}
        </Lazy>
      </section>
    {/if}
  </div>

  <section class="min-w-0">
    <div class="mb-2 flex items-center gap-2">
      <p class="flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted">
        <Wrench size={13} /> Known fixes
      </p>
      <div class="relative ml-auto w-56">
        <MagnifyingGlass size={12} class="pointer-events-none absolute left-2 top-1/2 -translate-y-1/2 text-muted" />
        <input
          bind:value={q}
          aria-label="Search fixes by symptom"
          placeholder="Describe the symptom…"
          class="h-7 w-full rounded-lg border border-border bg-surface-2 pl-7 pr-2 text-xs text-text outline-none focus:border-accent"
        />
      </div>
    </div>
    <div class="space-y-1">
      {#each fixes as f (f.index)}
        {@const r = app.recipes[f.index]}
        {@const accent = r.accent ?? "var(--accent)"}
        <div class="flex items-start gap-3 rounded-lg px-2.5 py-2 transition hover:bg-surface-2/50">
          <div class="min-w-0 flex-1">
            <p class="text-sm text-text">{r.symptom ?? r.name}</p>
            <p class="truncate text-xs text-muted" title={r.description}>
              {r.symptom ? r.name : r.description}
            </p>
          </div>
          <RecipePreview index={f.index} {accent} />
          <button
            onclick={() => void app.applyRecipe(f.index)}
            class="shrink-0 rounded-lg px-2.5 py-1 text-xs font-medium transition active:scale-95"
            style="background: color-mix(in srgb, {accent} 20%, transparent); color: {accent}"
          >
            Apply
          </button>
        </div>
      {:else}
        <p class="px-2.5 py-4 text-xs text-muted">
          {q.trim() ? `No fix matches “${q.trim()}”.` : "No fixes for this hardware."}
        </p>
      {/each}
    </div>
  </section>
</div>
