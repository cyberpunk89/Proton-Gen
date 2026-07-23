<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { app } from "$lib/state.svelte";
  import Header from "$lib/components/Header.svelte";
  import NavRail from "$lib/components/NavRail.svelte";
  import MainPanel from "$lib/components/MainPanel.svelte";
  import Library from "$lib/components/Library.svelte";
  import CommandPreview from "$lib/components/CommandPreview.svelte";
  import Notices from "$lib/components/Notices.svelte";
  import StaleBanner from "$lib/components/StaleBanner.svelte";
  import UpdateBanner from "$lib/components/UpdateBanner.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import { CircleNotch, WarningCircle } from "phosphor-svelte";

  onMount(() => {
    app.init();
  });
</script>

{#snippet loadErrorBanner()}
  {#if app.loadError}
    <div
      class="flex items-center gap-2 rounded-xl border px-4 py-2.5 text-xs text-red"
      style="border-color: color-mix(in srgb, var(--red) 35%, transparent); background: color-mix(in srgb, var(--red) 8%, transparent)"
    >
      <WarningCircle size={16} weight="fill" />
      {app.loadError}
    </div>
  {/if}
{/snippet}

{#if !app.ready}
  <div class="flex h-screen items-center justify-center">
    <CircleNotch size={28} class="animate-spin text-accent" />
  </div>
{:else}
  <div class="flex h-screen flex-col">
    <Header />

    {#if app.view === "library"}
      <div class="min-h-0 flex-1 overflow-y-auto" in:fade={{ duration: 120 }}>
        {#if app.loadError || app.staleVisible || app.updateVisible}
          <div class="mx-auto flex w-full max-w-6xl flex-col gap-3 px-6 pt-4">
            {@render loadErrorBanner()}
            <UpdateBanner />
            <StaleBanner />
          </div>
        {/if}
        <Library />
      </div>
    {:else}
      <div class="flex min-h-0 flex-1" in:fade={{ duration: 120 }}>
        <NavRail />
        <main class="flex min-h-0 min-w-0 flex-1 flex-col">
          <!-- The single scrolling region: nothing else clips. -->
          <div class="min-h-0 flex-1 overflow-y-auto">
            <div class="mx-auto flex max-w-4xl flex-col gap-3 px-5 py-4">
              {@render loadErrorBanner()}
              <UpdateBanner />
              <StaleBanner />
              <Notices />
              <MainPanel />
            </div>
          </div>

          <!-- Pinned command bar -->
          <div class="shrink-0 border-t border-border bg-mantle/30 px-5 py-3">
            <div class="mx-auto max-w-4xl">
              <CommandPreview />
            </div>
          </div>
        </main>
      </div>
    {/if}
  </div>
{/if}

<Toast />
