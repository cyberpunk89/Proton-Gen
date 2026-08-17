<script lang="ts">
  import { app } from "$lib/state.svelte";
  import Dialog from "./Dialog.svelte";

  /**
   * The "Apply to Heroic?" confirmation, mounted once at the app root.
   *
   * Deliberately not co-located with its trigger (`LauncherAction`): that
   * component is mounted at two call sites simultaneously, and both of them —
   * the pinned command bar and the Game & runtime section — unmount on an
   * ordinary view or section change. A bits-ui modal unmounted while open never
   * runs its own teardown, so `body { pointer-events: none }` survives it and
   * every click in the app stops working, with no error and nothing on screen
   * to explain it. Mounting it here means nothing can pull it out mid-flight.
   */
  async function applyHeroic() {
    app.heroicConfirmOpen = false;
    await app.injectHeroic();
  }
</script>

<Dialog
  bind:open={app.heroicConfirmOpen}
  title="Apply to Heroic?"
  subtitle="Close Heroic first — it may overwrite these changes when it exits."
>
  <div class="space-y-4">
    <p class="text-sm text-subtext">
      protongen will write the selected environment variables and wrappers into this
      game's Heroic config, replacing any it wrote before. A timestamped backup is saved
      next to the file first. Heroic's own settings — Proton version, prefix, sync
      options — are left untouched.
    </p>
    <div class="flex justify-end gap-2">
      <button
        onclick={() => (app.heroicConfirmOpen = false)}
        class="rounded-lg border border-border bg-surface-2/60 px-3 py-1.5 text-xs text-subtext transition hover:border-accent/50"
      >
        Cancel
      </button>
      <button
        onclick={applyHeroic}
        class="rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-on-accent transition hover:opacity-90"
      >
        Apply
      </button>
    </div>
  </div>
</Dialog>
