<script lang="ts">
  import { app } from "$lib/state.svelte";
  import Dialog from "./Dialog.svelte";
  import { GlobeHemisphereWest } from "phosphor-svelte";

  /**
   * "Apply your default profile?" — shown when a game with no saved config is
   * opened and the user has authored a global (default) profile in Settings.
   *
   * Mounted once at the app root, like HeroicConfirm: it's driven by an
   * ephemeral store flag rather than a trigger inside a view, so a view/section
   * change can't unmount a bits-ui modal mid-flight (which would leave
   * `body { pointer-events: none }` behind and click-kill the app — #63/HeroicConfirm).
   */
  const gp = $derived(app.store.global_profile);
  const summary = $derived(
    gp ? `${gp.env.length} env · ${gp.wrappers.length} wrappers` : "",
  );

  function apply() {
    app.pendingDefaultPrompt = false;
    app.applyGlobalProfile();
  }
</script>

<Dialog
  bind:open={app.pendingDefaultPrompt}
  title="Apply your default profile?"
  subtitle={app.selectedGameName
    ? `${app.selectedGameName} has no saved tuning yet.`
    : "This game has no saved tuning yet."}
  width="26rem"
>
  <div class="space-y-4">
    <div
      class="flex items-center gap-2.5 rounded-lg border border-accent/40 bg-accent/5 px-3 py-2.5 text-sm text-subtext"
    >
      <GlobeHemisphereWest size={16} class="shrink-0 text-accent" />
      <span>Your default profile: <span class="font-medium text-text">{summary}</span></span>
    </div>
    <p class="text-sm text-muted">
      Apply it as this game's starting point, or start clean — you can always apply it
      later from the Presets menu.
    </p>
    <div class="flex justify-end gap-2">
      <button
        onclick={() => (app.pendingDefaultPrompt = false)}
        class="rounded-lg border border-border bg-surface-2/60 px-3 py-1.5 text-xs text-subtext transition hover:border-accent/50"
      >
        Start clean
      </button>
      <button
        onclick={apply}
        class="rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-on-accent transition hover:opacity-90"
      >
        Apply default
      </button>
    </div>
  </div>
</Dialog>
