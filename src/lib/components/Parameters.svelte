<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { matches, irrelevance } from "$lib/util";
  import OptionRow from "./OptionRow.svelte";
  import { MagnifyingGlass, X, CaretRight, Faders } from "phosphor-svelte";
  import { slide } from "svelte/transition";
  import type { EnvDef, WrapperDef } from "$lib/types";

  let filter = $state("");
  let collapsed = $state<Record<string, boolean>>({});

  const filtering = $derived(filter.trim().length > 0);
  let showAll = $derived(app.store.show_irrelevant);

  function isOpen(cat: string): boolean {
    if (filtering) return true;
    return !collapsed[cat];
  }
  function toggle(cat: string) {
    collapsed[cat] = !collapsed[cat];
  }

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

  let wrappersShown = $derived(
    app.catalog.wrappers.filter(
      (w) =>
        matches(filter, [w.key, w.label ?? "", w.help, w.details ?? ""]) &&
        (showAll || !reason(w)),
    ),
  );

  function envsIn(cat: string) {
    return app.catalog.envs.filter(
      (e) =>
        e.category === cat &&
        matches(filter, [e.key, e.help, e.details ?? "", e.category]) &&
        (showAll || !reason(e)),
    );
  }

  let visibleCategories = $derived(
    app.categories.filter((c) => envsIn(c).length > 0),
  );
  let anyResults = $derived(
    wrappersShown.length > 0 || visibleCategories.length > 0,
  );

  // Search-matched options that don't apply to the detected hardware (hidden
  // unless "show all" is on). Independent of showAll so the toggle line only
  // appears when there's actually something to hide/show.
  let irrelevantCount = $derived.by(() => {
    const w = app.catalog.wrappers.filter(
      (it: WrapperDef) =>
        matches(filter, [it.key, it.label ?? "", it.help, it.details ?? ""]) &&
        !!reason(it),
    ).length;
    const e = app.catalog.envs.filter(
      (it: EnvDef) =>
        matches(filter, [it.key, it.help, it.details ?? "", it.category]) &&
        !!reason(it),
    ).length;
    return w + e;
  });
</script>

<section class="card p-4">
  <div class="mb-3 flex items-center gap-3">
    <Faders size={18} class="text-accent" />
    <h2 class="text-sm font-medium tracking-wide text-text">Parameters</h2>
    <div class="relative ml-auto w-72 max-w-[55%]">
      <MagnifyingGlass
        size={15}
        class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted"
      />
      <input
        bind:value={filter}
        placeholder="Search parameters…"
        class="w-full rounded-lg border border-border bg-surface-2 py-1.5 pl-9 pr-8 text-sm text-text outline-none focus:border-accent"
      />
      {#if filter}
        <button
          onclick={() => (filter = "")}
          class="absolute right-2 top-1/2 -translate-y-1/2 text-muted hover:text-text"
          aria-label="Clear search"
        >
          <X size={14} />
        </button>
      {/if}
    </div>
  </div>

  {#if irrelevantCount > 0}
    <div class="mb-3 flex items-center gap-2 text-xs text-muted">
      {#if showAll}
        <span>Showing all parameters, including unsupported ones.</span>
        <button
          onclick={() => app.setShowIrrelevant(false)}
          class="font-medium text-accent hover:underline">Hide unsupported</button
        >
      {:else}
        <span>{irrelevantCount} hidden for your hardware.</span>
        <button
          onclick={() => app.setShowIrrelevant(true)}
          class="font-medium text-accent hover:underline">Show all</button
        >
      {/if}
    </div>
  {/if}

  {#if !anyResults}
    <div class="flex flex-col items-center gap-2 py-12 text-center">
      <MagnifyingGlass size={26} class="text-muted" />
      <p class="text-sm text-muted">No parameters match “{filter}”.</p>
      <button
        onclick={() => (filter = "")}
        class="text-xs text-accent hover:underline">Clear search</button
      >
    </div>
  {:else}
    <div class="space-y-2.5">
      <!-- Wrappers -->
      {#if wrappersShown.length}
        {@render category("Wrappers")}
      {/if}

      {#each visibleCategories as cat (cat)}
        {@render category(cat)}
      {/each}
    </div>
  {/if}
</section>

{#snippet category(title: string)}
  {@const isWrappers = title === "Wrappers"}
  {@const onCount = isWrappers
    ? app.catalog.wrappers.filter((w) => app.wrap[w.key]?.enabled).length
    : app.enabledCountInCategory(title)}
  <div class="overflow-hidden rounded-xl border border-border/60 bg-surface-solid/40">
    <button
      onclick={() => toggle(title)}
      class="flex w-full items-center gap-2 px-3 py-2.5 text-left"
    >
      <CaretRight
        size={14}
        weight="bold"
        class="text-muted transition-transform duration-200 {isOpen(title)
          ? 'rotate-90'
          : ''}"
      />
      <span class="text-sm font-medium text-text">{title}</span>
      {#if onCount > 0}
        <span
          class="rounded-full px-2 py-0.5 text-[11px]"
          style="background: color-mix(in srgb, var(--accent) 18%, transparent); color: var(--accent)"
          >{onCount} on</span
        >
      {/if}
    </button>

    {#if isOpen(title)}
      <div class="space-y-0.5 px-2 pb-2" transition:slide={{ duration: 180 }}>
        {#if isWrappers}
          {#each wrappersShown as w (w.key)}
            <OptionRow
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
        {:else}
          {#each envsIn(title) as e (e.key)}
            <OptionRow
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
        {/if}
      </div>
    {/if}
  </div>
{/snippet}
