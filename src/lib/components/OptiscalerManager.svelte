<script lang="ts">
  import { untrack } from "svelte";
  import { app } from "$lib/state.svelte";
  import { openUrl } from "$lib/util";
  import type { OptiscalerInstall } from "$lib/types";
  import Dialog from "./Dialog.svelte";
  import {
    ArrowCircleUp,
    ArrowClockwise,
    ArrowSquareOut,
    CheckCircle,
    CircleDashed,
    Flask,
    Info,
  } from "phosphor-svelte";

  /**
   * Every OptiScaler in the library and whether it's current. Read-only: a
   * manual install that's behind opens its game's OptiScaler builder, where the
   * confirm-gated fetch lives — this dialog never writes anything itself.
   */
  // Re-check on every open. Untracked: the check reads (and flips) its own
  // loading flag, which would otherwise re-run this effect forever.
  $effect(() => {
    if (app.optiscalerManagerOpen) untrack(() => void app.checkOptiscalerOverview());
  });

  let overview = $derived(app.optiscalerOverview);

  // Things to act on first, then Proton's copies, then the rest by name.
  const RANK: Record<OptiscalerInstall["freshness"], number> = {
    outdated: 0,
    "proton-refresh": 1,
    "proton-waiting": 2,
    unknown: 3,
    ahead: 4,
    current: 5,
  };
  let rows = $derived(
    (overview?.installs ?? [])
      .map((i) => ({ i, name: app.games.find((g) => g.app_id === i.app_id)?.name ?? `Game ${i.app_id}` }))
      .sort((a, b) => RANK[a.i.freshness] - RANK[b.i.freshness] || a.name.localeCompare(b.name)),
  );

  function status(i: OptiscalerInstall): string {
    switch (i.freshness) {
      case "current":
        return "Up to date";
      case "outdated":
        return `Update available → ${i.latest}`;
      case "ahead":
        return "Newer than any published build";
      case "proton-refresh":
        return `Proton swaps in ${i.latest} on the next launch`;
      case "proton-waiting":
        return `OptiScaler ${i.latest} is out; Proton picks it up once proton-upscalers packages it`;
      case "unknown":
        return i.build ? "Couldn't compare with upstream" : "Version not readable";
    }
  }
</script>

<Dialog
  bind:open={app.optiscalerManagerOpen}
  title="OptiScaler versions"
  subtitle="Every OptiScaler in your library, against the newest upstream build."
  width="42rem"
>
  {#if !overview}
    <p class="flex items-center justify-center gap-1.5 py-6 text-sm text-muted">
      <ArrowClockwise size={14} class="animate-spin" /> Checking game folders and prefixes…
    </p>
  {:else}
    <div class="mb-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted">
      {#if overview.stable}
        {@const s = overview.stable}
        <button onclick={() => openUrl(s.html_url)} class="inline-flex items-center gap-1 hover:text-text">
          Latest stable <span class="font-medium text-subtext">{s.version}</span>
          <ArrowSquareOut size={11} />
        </button>
      {/if}
      {#if overview.nightly}
        {@const n = overview.nightly}
        <button onclick={() => openUrl(n.html_url)} class="inline-flex items-center gap-1 hover:text-text">
          Latest nightly <span class="font-medium text-subtext">{n.version}{n.built ? ` (${n.built})` : ""}</span>
          <ArrowSquareOut size={11} />
        </button>
      {/if}
      {#if overview.proton_latest}
        <span>Proton injects <span class="font-medium text-subtext">{overview.proton_latest}</span></span>
      {/if}
      <button
        onclick={() => app.checkOptiscalerOverview()}
        disabled={app.optiscalerOverviewLoading}
        class="ml-auto inline-flex items-center gap-1 hover:text-text disabled:opacity-50"
      >
        <ArrowClockwise size={11} class={app.optiscalerOverviewLoading ? "animate-spin" : ""} /> Re-check
      </button>
    </div>

    {#if !rows.length}
      <p class="py-6 text-center text-sm text-muted">
        No OptiScaler found — neither in a game folder nor injected by Proton in a prefix.
      </p>
    {:else}
      <ul class="max-h-[55vh] space-y-1.5 overflow-y-auto">
        {#each rows as { i, name } (`${i.app_id}-${i.kind}`)}
          {@const act = i.freshness === "outdated" && i.kind === "manual"}
          <li
            class="flex items-center gap-3 rounded-lg border px-3 py-2 text-sm {act
              ? 'border-accent/50 bg-accent/5'
              : 'border-border/60'}"
          >
            {#if i.freshness === "current"}
              <CheckCircle size={16} weight="fill" class="shrink-0 text-green" />
            {:else if act}
              <ArrowCircleUp size={16} weight="fill" class="shrink-0 text-accent" />
            {:else if i.freshness === "ahead"}
              <Flask size={16} class="shrink-0 text-yellow" />
            {:else if i.freshness === "unknown"}
              <Info size={16} class="shrink-0 text-muted" />
            {:else}
              <CircleDashed size={16} weight="bold" class="shrink-0 text-muted" />
            {/if}
            <div class="min-w-0 flex-1">
              <p class="truncate text-text">
                {name}
                <span class="ml-1 rounded bg-surface-2 px-1 text-[10px] uppercase tracking-wide text-muted"
                  >{i.kind === "manual" ? "game folder" : "Proton"}</span
                >
              </p>
              <p class="truncate text-xs text-muted" title={i.dll ? `${i.dir}/${i.dll}` : i.dir}>
                <span class="font-mono text-subtext">{i.build?.version ?? "?"}</span>{#if i.build?.built}
                  <span> ({i.build.built})</span>{/if}{#if i.kind === "manual" && i.dll}
                  · as <span class="font-mono">{i.dll}</span>{/if} · {status(i)}
              </p>
            </div>
            <button
              onclick={() => app.openOptiscalerFor(i.app_id)}
              class="inline-flex shrink-0 items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs font-medium transition active:scale-95 {act
                ? ''
                : 'border border-border bg-surface-2/50 text-subtext hover:border-accent/50'}"
              style={act ? "background: var(--accent); color: var(--on-accent)" : undefined}
            >
              {act ? "Upgrade…" : "Open"}
            </button>
          </li>
        {/each}
      </ul>
      <p class="mt-3 text-[11px] leading-snug text-muted">
        Game-folder installs only change when you upgrade them. Proton's copy (from
        <span class="font-mono">PROTON_USE_OPTISCALER</span>) is re-synced by proton-cachyos at every
        launch, and stays in the prefix even after injection is turned off.
      </p>
    {/if}
    {#if overview.errors.length}
      <p class="mt-2 text-[11px] text-yellow">Couldn't check: {overview.errors.join("; ")}</p>
    {/if}
  {/if}
</Dialog>
