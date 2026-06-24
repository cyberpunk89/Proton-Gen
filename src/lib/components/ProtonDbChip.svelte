<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { ipc } from "$lib/ipc";
  import { openUrl, tierColor } from "$lib/util";
  import type { Tier } from "$lib/types";
  import { Trophy, ArrowSquareOut, ArrowClockwise } from "phosphor-svelte";

  type Status = "idle" | "loading" | "done" | "failed";
  let status = $state<Status>("idle");
  let tier = $state<Tier | null>(null);
  let error = $state("");
  let lastAppId = $state<number | null>(null);

  // Reset when the selected game changes; auto-fetch if the user opted in.
  $effect(() => {
    if (app.selectedAppId !== lastAppId) {
      lastAppId = app.selectedAppId;
      status = "idle";
      tier = null;
      error = "";
      if (app.store.protondb_auto && app.selectedAppId != null) {
        fetchTier();
      }
    }
  });

  async function fetchTier() {
    if (app.selectedAppId == null) return;
    status = "loading";
    try {
      tier = await ipc.protondbFetch(app.selectedAppId);
      status = "done";
    } catch (e) {
      error = String(e);
      status = "failed";
    }
  }

  async function open() {
    if (app.selectedAppId == null) return;
    openUrl(await ipc.protondbUrl(app.selectedAppId));
  }
</script>

{#if status === "idle"}
  <button
    onclick={fetchTier}
    class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/60 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50"
  >
    <Trophy size={13} /> ProtonDB
  </button>
{:else if status === "loading"}
  <span class="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs text-muted">
    <ArrowClockwise size={13} class="animate-spin" /> Checking…
  </span>
{:else if status === "done" && tier}
  <div class="inline-flex items-center gap-2" title={`${tier.confidence} (${tier.total} reports)`}>
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
{:else}
  <button
    onclick={fetchTier}
    class="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs text-red"
    title={error}
  >
    <ArrowClockwise size={13} /> Retry
  </button>
{/if}
