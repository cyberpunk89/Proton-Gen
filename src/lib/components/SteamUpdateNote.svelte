<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { ArrowsClockwise, DownloadSimple } from "phosphor-svelte";

  /**
   * What Steam is doing to this game underneath its tuning, from the
   * appmanifest (`GameDto.update_pending` / `build_id`):
   * - updated since the tuning was last saved → worth re-checking, because an
   *   update can change what a launch option does (a new renderer, a bundled
   *   DLSS, the bug a workaround was for fixed). "Checked" stamps the current
   *   build without touching the tuning.
   * - an update pending/downloading → the next launch runs a different build.
   */
  let game = $derived(app.selectedGame);
  let updated = $derived(app.updatedSinceTuned(game));
  let pending = $derived(game?.source === "steam" && game.update_pending);

  function when(ts: number | null | undefined): string {
    if (!ts) return "";
    const days = Math.floor((Date.now() / 1000 - ts) / 86400);
    if (days <= 0) return " today";
    if (days === 1) return " yesterday";
    if (days < 30) return ` ${days} days ago`;
    return ` on ${new Date(ts * 1000).toLocaleDateString()}`;
  }
</script>

{#if game && (updated || pending)}
  <div class="space-y-1.5">
    {#if updated}
      <div
        class="flex items-start gap-2 rounded-lg border px-3 py-2 text-xs"
        style="border-color: color-mix(in srgb, var(--peach) 35%, transparent); background: color-mix(in srgb, var(--peach) 7%, transparent)"
        role="status"
      >
        <ArrowsClockwise size={14} class="mt-0.5 shrink-0 text-peach" />
        <p class="flex-1 text-subtext">
          Steam updated this game{when(game.last_updated)}, after you tuned it. Worth a
          quick test that your options still help.
        </p>
        <button
          onclick={() => app.acknowledgeUpdate(game)}
          class="shrink-0 rounded-md bg-surface-2 px-2 py-1 text-xs text-subtext transition hover:text-text"
          title="Mark the current build as checked; keeps your tuning as it is"
        >
          Checked
        </button>
      </div>
    {/if}
    {#if pending}
      <p class="flex items-center gap-1.5 text-xs text-muted">
        <DownloadSimple size={13} class="shrink-0" />
        Steam has an update pending for this game — the next launch may run a newer build.
      </p>
    {/if}
  </div>
{/if}
