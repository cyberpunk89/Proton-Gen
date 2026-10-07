<script lang="ts">
  import { app } from "$lib/state.svelte";
  import Dialog from "./Dialog.svelte";

  /**
   * The "Apply to Nexus?" confirmation, mounted once at the app root for the
   * same reason as `HeroicConfirm`: its trigger (`LauncherAction`) is mounted
   * at two call sites that both unmount on routine view changes, and a bits-ui
   * modal unmounted while open strands `body { pointer-events: none }` (#63).
   */
  async function apply() {
    app.nexusConfirmOpen = false;
    await app.applyToNexus();
  }
</script>

<Dialog
  bind:open={app.nexusConfirmOpen}
  title="Apply to Nexus?"
  subtitle="Nexus will launch {app.selectedGameName ?? 'this game'} with this tuning."
>
  <div class="space-y-4">
    <p class="text-sm text-subtext">
      protongen hands these environment variables, wrappers, game arguments and Proton
      version to Nexus as the game's launch profile (<span class="font-mono text-xs"
        >nexus-cli --set-launch</span
      >). Nexus then rewrites what it keeps for this game — its launch script, desktop
      entry, and its Steam shortcut and Heroic entry — replacing the previous profile.
      The game's exe and prefix stay Nexus's own.
    </p>
    <div class="flex justify-end gap-2">
      <button
        onclick={() => (app.nexusConfirmOpen = false)}
        class="rounded-lg border border-border bg-surface-2/60 px-3 py-1.5 text-xs text-subtext transition hover:border-accent/50"
      >
        Cancel
      </button>
      <button
        onclick={apply}
        class="rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-on-accent transition hover:opacity-90"
      >
        Apply
      </button>
    </div>
  </div>
</Dialog>
