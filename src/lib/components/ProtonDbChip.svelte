<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { ipc } from "$lib/ipc";
  import { openUrl, tierColor } from "$lib/util";
  import { Trophy, ArrowSquareOut, ArrowClockwise } from "phosphor-svelte";

  /**
   * Purely presentational now: the fetch, the de-duping and the result all live
   * in the store's session cache (`app.requestTier`). Previously this component
   * held the tier in local state and refetched on every game change, so bouncing
   * between two games hit protondb.com four times instead of twice.
   */

  let appId = $derived(app.selectedAppId);
  let tier = $derived(appId == null ? undefined : app.tierFor(appId));
  let loading = $derived(appId != null && app.tierLoading[String(appId)] === true);

  // Auto-fetch on selection when the user opted in. The cache makes a repeat
  // visit free, so this no longer needs a "did the game change" guard.
  $effect(() => {
    if (app.store.protondb_auto && appId != null) app.requestTier(appId);
  });

  async function open() {
    if (appId == null) return;
    openUrl(await ipc.protondbUrl(appId));
  }
</script>

{#if appId == null}
  <!-- nothing to look up -->
{:else if loading}
  <span class="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs text-muted">
    <ArrowClockwise size={13} class="animate-spin" /> Checking…
  </span>
{:else if tier}
  <div
    class="inline-flex items-center gap-2"
    title={`${tier.confidence} (${tier.total} reports)`}
  >
    <span
      class="rounded-full px-2.5 py-1 text-xs font-medium capitalize"
      style="background: {tierColor(tier.tier)}; color: #11111b"
    >
      {tier.tier}
    </span>
    <span class="text-xs text-muted">{tier.total} reports</span>
    <button onclick={open} class="text-muted hover:text-blue" aria-label="Open ProtonDB page">
      <ArrowSquareOut size={14} />
    </button>
  </div>
{:else if tier === null}
  <!-- Cached failure: offer a retry rather than silently hiding. -->
  <button
    onclick={() => app.retryTier(appId)}
    class="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs text-red"
    title="Couldn't reach ProtonDB"
  >
    <ArrowClockwise size={13} /> Retry
  </button>
{:else}
  <button
    onclick={() => app.requestTier(appId)}
    class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/60 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50"
  >
    <Trophy size={13} /> ProtonDB
  </button>
{/if}
