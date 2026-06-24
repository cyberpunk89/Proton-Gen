<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { copyText } from "$lib/util";
  import { toast } from "$lib/toast.svelte";
  import { ArrowsClockwise, Copy, X } from "phosphor-svelte";

  async function copyCmd() {
    await copyText("/update-proton-params");
    toast.show("Copied");
  }
</script>

{#if app.staleVisible && app.stale}
  <div
    class="flex items-center gap-3 rounded-xl border px-4 py-2.5"
    style="border-color: color-mix(in srgb, var(--yellow) 35%, transparent);
           background: color-mix(in srgb, var(--yellow) 8%, transparent)"
  >
    <ArrowsClockwise size={16} class="shrink-0 text-yellow" />
    <p class="flex-1 text-xs text-subtext">
      proton-cachyos <span class="font-medium text-text">{app.stale.installed}</span>
      installed, but the catalog was refreshed for
      <span class="font-medium text-text">{app.stale.catalog}</span>.
      Run <code class="font-mono text-yellow">/update-proton-params</code> to refresh.
    </p>
    <button
      onclick={copyCmd}
      class="inline-flex items-center gap-1 rounded-lg bg-surface-2 px-2 py-1 text-xs text-subtext hover:text-text"
    >
      <Copy size={12} /> Copy
    </button>
    <button
      onclick={() => app.dismissStale()}
      class="text-muted hover:text-text"
      aria-label="Dismiss"
    >
      <X size={14} />
    </button>
  </div>
{/if}
