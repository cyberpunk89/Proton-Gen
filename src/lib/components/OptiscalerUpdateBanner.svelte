<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { ArrowCircleUp, X } from "phosphor-svelte";

  // Report-only, like RuntimeUpdateBanner: the upgrade itself stays behind the
  // per-game confirm in the OptiScaler builder, so this only points there.
  // Proton's injected copies never raise it — Proton refreshes those itself.
  let outdated = $derived(app.optiscalerOutdated);
  let names = $derived(
    outdated.map((i) => app.games.find((g) => g.app_id === i.app_id)?.name ?? `Game ${i.app_id}`),
  );
  let latest = $derived([...new Set(outdated.map((i) => i.latest).filter(Boolean))].join(" / "));
</script>

{#if app.optiscalerNoticeVisible}
  <div
    class="flex items-center gap-3 rounded-xl border px-4 py-2.5"
    style="border-color: color-mix(in srgb, var(--accent) 35%, transparent);
           background: color-mix(in srgb, var(--accent) 8%, transparent)"
  >
    <ArrowCircleUp size={16} class="shrink-0 text-accent" />
    <p class="flex-1 text-xs text-subtext">
      <span class="font-medium text-text">OptiScaler {latest}</span> is out —
      {#if outdated.length === 1}
        <span class="font-medium text-text">{names[0]}</span> still has
        {outdated[0].build?.version ?? "an older build"} in its game folder.
      {:else}
        {outdated.length} games still have an older build in their game folder
        ({names.slice(0, 3).join(", ")}{names.length > 3 ? ", …" : ""}).
      {/if}
    </p>
    <button
      onclick={() => (app.optiscalerManagerOpen = true)}
      class="shrink-0 rounded-lg bg-surface-2 px-2 py-1 text-xs text-subtext transition hover:text-text"
    >
      Review…
    </button>
    <button
      onclick={() => app.dismissOptiscalerUpdate()}
      class="shrink-0 text-muted hover:text-text"
      aria-label="Dismiss OptiScaler {latest} notice"
    >
      <X size={14} />
    </button>
  </div>
{/if}
