<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { openUrl } from "$lib/util";
  import { toast } from "$lib/toast.svelte";
  import { DownloadSimple, ArrowSquareOut, X, CircleNotch } from "phosphor-svelte";

  async function apply() {
    try {
      await app.applyUpdate();
    } catch (e) {
      toast.show(`Update failed: ${e}`);
    }
  }
</script>

{#if app.updateVisible && app.update}
  <div
    class="flex items-center gap-3 rounded-xl border px-4 py-2.5"
    style="border-color: color-mix(in srgb, var(--green) 35%, transparent);
           background: color-mix(in srgb, var(--green) 8%, transparent)"
  >
    <DownloadSimple size={16} class="shrink-0 text-green" />
    <p class="flex-1 text-xs text-subtext">
      Update available — <span class="font-medium text-text">v{app.update.current}</span>
      → <span class="font-medium text-text">v{app.update.latest}</span>.
      {#if app.update.html_url}
        <button
          onclick={() => app.update && openUrl(app.update.html_url)}
          class="ml-1 inline-flex items-center gap-1 align-baseline text-green hover:underline"
        >
          release notes <ArrowSquareOut size={11} />
        </button>
      {/if}
    </p>
    <button
      onclick={apply}
      disabled={app.updating}
      class="inline-flex items-center gap-1 rounded-lg px-2.5 py-1 text-xs font-medium transition active:scale-95 disabled:opacity-60"
      style="background: var(--accent); color: var(--on-accent)"
    >
      {#if app.updating}
        <CircleNotch size={12} class="animate-spin" /> Updating…
      {:else}
        <DownloadSimple size={12} /> Update now
      {/if}
    </button>
    <button
      onclick={() => app.dismissUpdate()}
      class="text-muted hover:text-text"
      aria-label="Dismiss"
    >
      <X size={14} />
    </button>
  </div>
{/if}
