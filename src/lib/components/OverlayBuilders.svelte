<script lang="ts">
  import { app } from "$lib/state.svelte";
  import Dialog from "./Dialog.svelte";
  import Lazy from "./Lazy.svelte";
  import { lazy } from "$lib/lazy";

  /**
   * The MangoHud, OptiScaler, vkBasalt and Lossless Scaling builders, mounted once at
   * the app root.
   *
   * Both used to be defined twice — once inside SimplePanel, once inside
   * MainPanel, each with its own local `$state` + `<Dialog>` — because each
   * panel needed a "Configure…" entry point. But the Simple/Advanced toggle
   * unmounts whichever panel is showing (see App.svelte), and a bits-ui modal
   * unmounted while open never runs its own teardown: `body { pointer-events:
   * none }` survives it and every click in the app stops working, with no
   * error and nothing on screen to explain it. Same #63 failure mode as
   * `HeroicConfirm`, same fix: one dialog, driven by store state, mounted
   * somewhere that can't be pulled out from under it.
   *
   * The builder bodies are lazy chunks: a dialog's content only renders while
   * it is open, so each body loads on first open (normally already prefetched).
   */
</script>

<Dialog
  bind:open={app.mangoBuilderOpen}
  title="MangoHud overlay"
  subtitle="Build the overlay, then apply it to the launch command."
  width="46rem"
>
  <Lazy load={lazy.mangohud}>
    {#snippet children(C)}<C onapply={() => (app.mangoBuilderOpen = false)} />{/snippet}
  </Lazy>
</Dialog>

<Dialog
  bind:open={app.optiBuilderOpen}
  title="OptiScaler"
  subtitle="Compose the upscaler config, then apply it to the launch command."
  width="46rem"
>
  <Lazy load={lazy.optiscaler}>
    {#snippet children(C)}<C onapply={() => (app.optiBuilderOpen = false)} />{/snippet}
  </Lazy>
</Dialog>

<Dialog
  bind:open={app.vkBuilderOpen}
  title="vkBasalt effect chain"
  subtitle="Compose the effect chain, then set it as the system-wide vkBasalt.conf."
  width="46rem"
>
  <Lazy load={lazy.vkbasalt}>
    {#snippet children(C)}<C onapply={() => (app.vkBuilderOpen = false)} />{/snippet}
  </Lazy>
</Dialog>

<Dialog
  bind:open={app.lsfgBuilderOpen}
  title="Lossless Scaling frame generation"
  subtitle="Set up lsfg-vk for this game, then apply it to the launch command."
  width="46rem"
>
  <Lazy load={lazy.lsfg}>
    {#snippet children(C)}<C onapply={() => (app.lsfgBuilderOpen = false)} />{/snippet}
  </Lazy>
</Dialog>

<Dialog
  bind:open={app.gamescopeBuilderOpen}
  title="gamescope"
  subtitle="Pick output and render sizes, upscaling and display options for the gamescope wrapper."
  width="50rem"
>
  <Lazy load={lazy.gamescope}>
    {#snippet children(C)}<C onapply={() => (app.gamescopeBuilderOpen = false)} />{/snippet}
  </Lazy>
</Dialog>

<Dialog
  bind:open={app.dllBuilderOpen}
  title="DLL overrides"
  subtitle="Choose which DLLs Wine loads from the game folder, from Wine, or not at all."
  width="40rem"
>
  <Lazy load={lazy.dllOverrides}>
    {#snippet children(C)}<C onapply={() => (app.dllBuilderOpen = false)} />{/snippet}
  </Lazy>
</Dialog>
