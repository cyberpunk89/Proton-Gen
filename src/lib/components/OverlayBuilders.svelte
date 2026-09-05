<script lang="ts">
  import { app } from "$lib/state.svelte";
  import Dialog from "./Dialog.svelte";
  import MangoHud from "./MangoHud.svelte";
  import OptiScaler from "./OptiScaler.svelte";
  import VkBasalt from "./VkBasalt.svelte";

  /**
   * The MangoHud, OptiScaler and vkBasalt overlay builders, mounted once at
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
   */
</script>

<Dialog
  bind:open={app.mangoBuilderOpen}
  title="MangoHud overlay"
  subtitle="Build the overlay, then apply it to the launch command."
  width="46rem"
>
  <MangoHud onapply={() => (app.mangoBuilderOpen = false)} />
</Dialog>

<Dialog
  bind:open={app.optiBuilderOpen}
  title="OptiScaler"
  subtitle="Compose the upscaler config, then apply it to the launch command."
  width="46rem"
>
  <OptiScaler onapply={() => (app.optiBuilderOpen = false)} />
</Dialog>

<Dialog
  bind:open={app.vkBuilderOpen}
  title="vkBasalt effect chain"
  subtitle="Compose the effect chain, then set it as the system-wide vkBasalt.conf."
  width="46rem"
>
  <VkBasalt onapply={() => (app.vkBuilderOpen = false)} />
</Dialog>
