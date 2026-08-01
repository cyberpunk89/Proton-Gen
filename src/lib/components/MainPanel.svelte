<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { matches, irrelevance } from "$lib/util";
  import OptionRow from "./OptionRow.svelte";
  import Recipes from "./Recipes.svelte";
  import GameRuntimePanel from "./GameRuntimePanel.svelte";
  import { MagnifyingGlass, Faders } from "phosphor-svelte";
  import type { EnvDef, WrapperDef } from "$lib/types";

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

  // ---- the parameters shown for the current view (a category, or search) ----
  let envs = $derived(
    searching
      ? app.catalog.envs.filter(
          (e) =>
            matches(q, [e.key, e.help, e.details ?? "", e.category]) &&
            (showAll || !reason(e)),
        )
      : app.activeSection === "Wrappers"
        ? []
        : app.catalog.envs.filter(
            (e) =>
              e.category === app.activeSection && (showAll || !reason(e)),
          ),
  );
  let wraps = $derived(
    searching
      ? app.catalog.wrappers.filter(
          (w) =>
            matches(q, [w.key, w.label ?? "", w.help, w.details ?? ""]) &&
            (showAll || !reason(w)),
        )
      : app.activeSection === "Wrappers"
        ? app.catalog.wrappers.filter((w) => showAll || !reason(w))
        : [],
  );

  let anyShown = $derived(envs.length + wraps.length > 0);
  let title = $derived(
    searching ? `Results for “${q.trim()}”` : app.activeSection,
  );

  // Options hidden by hardware relevance in this view (drives the show-all line).
  let hiddenCount = $derived.by(() => {
    const wf = (w: WrapperDef) =>
      searching
        ? matches(q, [w.key, w.label ?? "", w.help, w.details ?? ""])
        : app.activeSection === "Wrappers";
    const ef = (e: EnvDef) =>
      searching
        ? matches(q, [e.key, e.help, e.details ?? "", e.category])
        : e.category === app.activeSection;
    const w = app.catalog.wrappers.filter((it) => wf(it) && !!reason(it)).length;
    const e = app.catalog.envs.filter((it) => ef(it) && !!reason(it)).length;
    return w + e;
  });
</script>

{#if searching}
  {@render optionsCard()}
{:else if app.activeSection === "recipes"}
  <Recipes />
{:else if app.activeSection === "game"}
  <GameRuntimePanel />
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
    {:else}
      <div class="space-y-0.5">
        {#each wraps as w (w.key)}
          <OptionRow
            paramKey={w.key}
            enabled={app.wrap[w.key]?.enabled ?? false}
            title={w.label ?? w.key}
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
            dim={!!reason(w)}
            onToggle={() => app.toggleWrap(w.key)}
            onValue={(v) => app.setWrapValue(w.key, v)}
          />
        {/each}
        {#each envs as e (e.key)}
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
            dim={!!reason(e)}
            onToggle={() => app.toggleEnv(e.key)}
            onValue={(v) => app.setEnvValue(e.key, v)}
          />
        {/each}
      </div>
    {/if}
  </section>
{/snippet}
