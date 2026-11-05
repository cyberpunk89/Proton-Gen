<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { compareConfigs, type CompareRow } from "$lib/compare";
  import {
    Pulse,
    ThumbsUp,
    ThumbsDown,
    ArrowCounterClockwise,
    FileText,
    Wrench,
    Sparkle,
  } from "phosphor-svelte";
  import type { Run } from "$lib/types";

  /**
   * The dock's Runs tab: every launch of this game protongen has seen, newest
   * first, each stamped with the config that ran, what its log said, and what
   * changed since the run before. protongen never launches — a run is
   * inferred when a log is written after you Copy / Apply (runs.rs).
   */
  let selectedId = $state<number | null>(null);
  let selected = $derived(app.runs.find((r) => r.id === selectedId) ?? app.runs[0] ?? null);
  let selectedIndex = $derived(selected ? app.runs.indexOf(selected) : -1);
  let previous = $derived(selectedIndex >= 0 ? (app.runs[selectedIndex + 1] ?? null) : null);

  /** What changed going from the run before to this one. */
  function changes(run: Run, before: Run | null): CompareRow[] {
    if (!before) return [];
    return compareConfigs(before.config, run.config).filter((r) => r.change !== "same");
  }

  function sign(r: CompareRow): string {
    return r.change === "only-b" ? "+" : r.change === "only-a" ? "−" : "~";
  }

  function when(secs: number): string {
    const d = new Date(secs * 1000);
    const now = new Date();
    const time = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    if (d.toDateString() === now.toDateString()) return time;
    const y = new Date(now);
    y.setDate(now.getDate() - 1);
    if (d.toDateString() === y.toDateString()) return `Yesterday ${time}`;
    return d.toLocaleDateString([], { month: "short", day: "numeric" });
  }

  function look(r: Run): { colour: string; label: string } {
    if (r.mark === true) return { colour: "var(--green)", label: "Worked" };
    if (r.mark === false) return { colour: "var(--red)", label: "Broke" };
    if (r.verdict === "crash") return { colour: "var(--red)", label: "Crashed" };
    if (r.verdict === "errors")
      return { colour: "var(--peach)", label: `${r.error_count} error${r.error_count === 1 ? "" : "s"}` };
    return { colour: "var(--green)", label: "Clean log" };
  }

  function recipeIndex(name: string): number {
    return app.recipes.findIndex((r) => r.name === name);
  }

  /** Steam games only write a Proton log with PROTON_LOG on. */
  let needsLogging = $derived(
    !app.umu && app.selectedGame?.source !== "nexus" && !app.env["PROTON_LOG"]?.enabled,
  );

  function enableLogging() {
    app.applyChanges("enable Proton logging", { enable: [["PROTON_LOG", "1"]] });
  }

  let good = $derived(app.lastGoodRun);
</script>

{#if app.selectedAppId == null}
  <p class="py-8 text-center text-sm text-muted">Runs are per game — pick one from the library.</p>
{:else}
  <div class="grid gap-4 md:grid-cols-[17rem_minmax(0,1fr)]">
    <!-- The list -->
    <div class="min-w-0 space-y-1">
      {#if needsLogging}
        <div class="mb-2 rounded-lg border border-yellow/40 bg-yellow/5 p-2.5 text-xs leading-snug text-subtext">
          Steam games only write a log with <span class="font-mono">PROTON_LOG=1</span>, so their runs
          can't be seen without it.
          <button onclick={enableLogging} class="mt-1 block font-medium text-accent hover:underline"
            >Turn on PROTON_LOG</button
          >
        </div>
      {/if}
      {#each app.runs as r, i (r.id)}
        {@const l = look(r)}
        {@const diff = changes(r, app.runs[i + 1] ?? null)}
        <button
          onclick={() => (selectedId = r.id)}
          aria-current={selected?.id === r.id ? "true" : undefined}
          class="w-full rounded-lg border px-3 py-2 text-left transition {selected?.id === r.id
            ? 'border-accent/50 bg-accent/5'
            : 'border-transparent hover:bg-surface-2/50'}"
        >
          <span class="flex items-center gap-2">
            <span class="size-2 shrink-0 rounded-full" style="background: {l.colour}"></span>
            <span class="text-sm text-text">{l.label}</span>
            {#if good?.id === r.id}
              <span class="rounded bg-green/15 px-1 text-[10px] text-green">last good</span>
            {/if}
            <span class="ml-auto shrink-0 text-[11px] text-muted">{when(r.log_at)}</span>
          </span>
          {#if diff.length}
            <span class="mt-0.5 block truncate pl-4 font-mono text-[11px] text-muted">
              {diff
                .slice(0, 3)
                .map((d) => `${sign(d)} ${d.label}`)
                .join("  ")}{diff.length > 3 ? `  +${diff.length - 3}` : ""}
            </span>
          {:else if i === app.runs.length - 1}
            <span class="mt-0.5 block pl-4 text-[11px] text-muted">first run seen</span>
          {:else}
            <span class="mt-0.5 block pl-4 text-[11px] text-muted">same config as before</span>
          {/if}
        </button>
      {:else}
        <div class="flex flex-col items-center gap-2 px-3 py-8 text-center">
          <Pulse size={24} class="text-muted" />
          <p class="text-sm text-muted">No runs yet.</p>
          <p class="text-xs leading-relaxed text-muted">
            Copy the command (or Apply to Nexus / Heroic), then launch the game. When its log is
            written, the run shows up here with the config that ran.
          </p>
        </div>
      {/each}
    </div>

    <!-- The selected run -->
    {#if selected}
      {@const l = look(selected)}
      {@const diff = changes(selected, previous)}
      <div class="min-w-0 space-y-3">
        <div class="flex flex-wrap items-center gap-2">
          <span class="size-2.5 rounded-full" style="background: {l.colour}"></span>
          <h3 class="text-sm font-medium text-text">{l.label}</h3>
          <span class="text-xs text-muted">· {selected.source} log · {when(selected.log_at)}</span>
          <div class="ml-auto inline-flex items-center gap-1">
            <button
              onclick={() => app.markRun(selected.id, selected.mark === true ? null : true)}
              aria-pressed={selected.mark === true}
              class="inline-flex items-center gap-1 rounded-lg border px-2 py-1 text-xs transition {selected.mark === true
                ? 'border-green/50 bg-green/10 text-green'
                : 'border-border text-subtext hover:border-green/50'}"
            >
              <ThumbsUp size={12} /> Worked
            </button>
            <button
              onclick={() => app.markRun(selected.id, selected.mark === false ? null : false)}
              aria-pressed={selected.mark === false}
              class="inline-flex items-center gap-1 rounded-lg border px-2 py-1 text-xs transition {selected.mark === false
                ? 'border-red/50 bg-red/10 text-red'
                : 'border-border text-subtext hover:border-red/50'}"
            >
              <ThumbsDown size={12} /> Broke
            </button>
          </div>
        </div>

        {#if selected.first_error}
          <pre
            class="overflow-x-auto rounded-lg border border-border bg-mantle/50 p-2.5 font-mono text-[11px] text-subtext">{selected.first_error}</pre>
        {/if}

        <div>
          <p class="mb-1 text-[11px] font-medium uppercase tracking-wider text-muted">
            {previous ? "Changed since the run before" : "Config that ran"}
          </p>
          {#if previous && diff.length}
            <ul class="space-y-0.5 font-mono text-[12px]">
              {#each diff as d (d.kind + d.label)}
                <li
                  class={d.change === "only-b" ? "text-green" : d.change === "only-a" ? "text-red" : "text-peach"}
                >
                  {sign(d)}
                  {d.label}{#if d.change === "changed"}<span class="text-muted">: {d.a} → {d.b}</span
                    >{:else if d.kind !== "wrapper" && (d.b ?? d.a)}<span class="text-muted"
                      >={d.b ?? d.a}</span
                    >{/if}
                </li>
              {/each}
            </ul>
          {:else if previous}
            <p class="text-xs text-muted">Nothing — same config as the run before.</p>
          {:else}
            <p class="break-all font-mono text-[11px] text-muted">
              {[
                ...selected.config.wrappers.map(([k]) => k),
                ...selected.config.env.map(([k, v]) => `${k}=${v}`),
              ].join(" ") || "defaults"}
            </p>
          {/if}
        </div>

        {#if selected.matched_recipes.length}
          <div>
            <p class="mb-1 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted">
              <Wrench size={12} /> Its log matches
            </p>
            {#each selected.matched_recipes as name (name)}
              {@const idx = recipeIndex(name)}
              <div class="flex items-center gap-2 rounded-lg px-2 py-1.5 hover:bg-surface-2/50">
                <Sparkle size={12} class="shrink-0 text-accent" />
                <span class="min-w-0 flex-1 truncate text-sm text-text">{name}</span>
                {#if idx >= 0}
                  <button
                    onclick={() => void app.applyRecipe(idx)}
                    class="shrink-0 rounded-lg border border-border px-2 py-0.5 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
                    >Apply</button
                  >
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        <div class="flex flex-wrap items-center gap-2 border-t border-border/60 pt-3">
          {#if good && good.id !== selected.id && (selected.verdict !== "clean" || selected.mark === false)}
            <button
              onclick={() => app.loadRun(good, "revert to the last good run")}
              class="inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition active:scale-95"
              style="background: var(--accent); color: var(--on-accent)"
            >
              <ArrowCounterClockwise size={13} /> Revert to last good run ({when(good.log_at)})
            </button>
          {/if}
          <button
            onclick={() => app.loadRun(selected)}
            class="inline-flex items-center gap-1.5 rounded-lg border border-border px-3 py-1.5 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
          >
            Load this run's config
          </button>
          <button
            onclick={() => app.openDock("logs")}
            class="inline-flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs text-muted transition hover:text-text"
          >
            <FileText size={13} /> Open log
          </button>
        </div>
      </div>
    {/if}
  </div>
{/if}
