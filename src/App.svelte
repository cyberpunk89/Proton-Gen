<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "$lib/state.svelte";
  import Header from "$lib/components/Header.svelte";
  import Hero from "$lib/components/Hero.svelte";
  import Recipes from "$lib/components/Recipes.svelte";
  import Parameters from "$lib/components/Parameters.svelte";
  import Notices from "$lib/components/Notices.svelte";
  import StaleBanner from "$lib/components/StaleBanner.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import { CircleNotch, WarningCircle } from "phosphor-svelte";

  onMount(() => {
    app.init();
  });
</script>

{#if !app.ready}
  <div class="flex h-screen items-center justify-center">
    <CircleNotch size={28} class="animate-spin text-accent" />
  </div>
{:else}
  <div class="flex h-screen flex-col">
    <Header />
    <main class="flex-1 overflow-y-auto">
      <div class="mx-auto flex max-w-5xl flex-col gap-4 px-5 pb-10 pt-1">
        {#if app.loadError}
          <div
            class="flex items-center gap-2 rounded-xl border px-4 py-2.5 text-xs text-red"
            style="border-color: color-mix(in srgb, var(--red) 35%, transparent); background: color-mix(in srgb, var(--red) 8%, transparent)"
          >
            <WarningCircle size={16} weight="fill" />
            {app.loadError}
          </div>
        {/if}

        <StaleBanner />
        <Hero />
        <Notices />
        <Recipes />
        <Parameters />
      </div>
    </main>
  </div>
{/if}

<Toast />
