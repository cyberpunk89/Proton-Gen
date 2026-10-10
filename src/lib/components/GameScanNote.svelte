<script lang="ts">
  import { untrack } from "svelte";
  import { app } from "$lib/state.svelte";
  import { lookups } from "$lib/lookups.svelte";
  import { MagnifyingGlass, ArrowsClockwise } from "phosphor-svelte";

  /**
   * What the selected game's own folder says about it (game_scan.rs): the
   * upscalers it bundles, kernel anti-cheat, the engine, and — for Unreal —
   * the folder the real exe lives in. This is what drives the "Suggested for
   * this game" recipes and the scan-aware anti-cheat lint, so it's shown
   * rather than kept as hidden magic.
   */
  let id = $derived(app.selectedAppId);
  let hasFolder = $derived(!!app.selectedGame?.install_dir);

  $effect(() => {
    if (id != null && hasFolder) lookups.requestScan(id);
  });

  let scan = $derived(lookups.scanFor(id));

  // Lint reads the backend's cached scan, so re-lint once one lands — or the
  // anti-cheat notices would wait for the next edit.
  $effect(() => {
    if (scan) untrack(() => app.retryBuild());
  });

  let facts = $derived(
    scan ? [...scan.upscalers, ...(scan.engine ? [scan.engine] : []), ...scan.anticheat] : [],
  );
</script>

{#if scan && (facts.length || scan.exe_dir)}
  <div class="flex items-start gap-2 text-xs text-muted">
    <MagnifyingGlass size={13} class="mt-0.5 shrink-0" />
    <div class="min-w-0 flex-1 space-y-0.5">
      {#if facts.length}
        <p>
          In the game folder:
          {#each facts as f, i (f)}
            <span
              class="font-medium {scan.anticheat.includes(f) ? 'text-peach' : 'text-subtext'}"
              >{f}</span
            >{i < facts.length - 1 ? " · " : ""}
          {/each}
        </p>
      {/if}
      {#if scan.exe_dir}
        <p class="truncate" title={scan.exe_dir}>
          Game exe lives in <code class="font-mono">{scan.exe_dir}</code> — DLL overrides and
          OptiScaler go there.
        </p>
      {/if}
    </div>
    <button
      onclick={() => id != null && lookups.requestScan(id, true)}
      class="shrink-0 rounded p-0.5 transition hover:text-text"
      aria-label="Scan the game folder again"
      title="Scan the game folder again"
    >
      <ArrowsClockwise size={12} />
    </button>
  </div>
{/if}
