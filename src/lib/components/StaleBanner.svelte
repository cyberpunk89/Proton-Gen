<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { openUrl } from "$lib/util";
  import { toast } from "$lib/toast.svelte";
  import { ArrowsClockwise, ArrowSquareOut, X, CircleNotch } from "phosphor-svelte";

  const HELP_URL =
    "https://github.com/cyberpunk89/Proton-Gen/wiki/Settings-and-files#customising-the-catalogue";

  let checking = $state(false);

  async function check() {
    checking = true;
    try {
      // A refreshed catalog ships inside a release, so updating the app is
      // the actual remedy here.
      const result = await app.checkForUpdate();
      if (result === "available") toast.success("Update available");
      else if (result === "up-to-date")
        toast.info("protongen is up to date — no newer catalog yet");
      else toast.error("Couldn't check for updates");
    } finally {
      checking = false;
    }
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
      is installed, but protongen's parameter catalog was last refreshed for
      <span class="font-medium text-text">{app.stale.catalog}</span>
      ({app.stale.updated}). Newer Proton options may be missing.
    </p>
    <button
      onclick={check}
      disabled={checking}
      class="inline-flex shrink-0 items-center gap-1 rounded-lg bg-surface-2 px-2 py-1 text-xs text-subtext transition hover:text-text disabled:opacity-60"
    >
      {#if checking}
        <CircleNotch size={12} class="animate-spin" /> Checking…
      {:else}
        <ArrowsClockwise size={12} /> Check for app update
      {/if}
    </button>
    <button
      onclick={() => openUrl(HELP_URL)}
      class="inline-flex shrink-0 items-center gap-1 rounded-lg bg-surface-2 px-2 py-1 text-xs text-subtext transition hover:text-text"
    >
      How to fix <ArrowSquareOut size={11} />
    </button>
    <button
      onclick={() => app.dismissStale()}
      class="shrink-0 text-muted hover:text-text"
      aria-label="Dismiss"
    >
      <X size={14} />
    </button>
  </div>
{/if}
