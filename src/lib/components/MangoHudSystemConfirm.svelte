<script lang="ts">
  import { app } from "$lib/state.svelte";
  import Dialog from "./Dialog.svelte";

  /**
   * The "Set as system MangoHud default?" confirmation, mounted once at the
   * app root — same rationale as `HeroicConfirm`: its trigger lives inside the
   * MangoHud dialog, which the user can close mid-flow, and a bits-ui modal
   * unmounted while open never runs its own teardown (stranding
   * `body { pointer-events: none }`). Mounting it here means nothing can pull
   * it out from under an open confirmation.
   */
  async function applyExport() {
    app.mangoSystemConfirmOpen = false;
    await app.exportMangoSystemWide();
  }
</script>

<Dialog
  bind:open={app.mangoSystemConfirmOpen}
  title="Set as system MangoHud default?"
  subtitle="Applies to every MangoHud-enabled game or app on this system, not just this one."
>
  <div class="space-y-4">
    <p class="text-sm text-subtext">
      protongen will merge these overlay settings into
      <code class="rounded bg-surface-2/60 px-1 py-0.5 text-xs">~/.config/MangoHud/MangoHud.conf</code>
      — the file every MangoHud-enabled program reads by default. A timestamped backup is
      saved first. Your font, keybinds, app blacklist, and anything else this app's overlay
      builder doesn't manage are left untouched; only the metrics, layout, and colors you've
      configured here are changed.
    </p>
    <div class="flex justify-end gap-2">
      <button
        onclick={() => (app.mangoSystemConfirmOpen = false)}
        class="rounded-lg border border-border bg-surface-2/60 px-3 py-1.5 text-xs text-subtext transition hover:border-accent/50"
      >
        Cancel
      </button>
      <button
        onclick={applyExport}
        class="rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-on-accent transition hover:opacity-90"
      >
        Apply
      </button>
    </div>
  </div>
</Dialog>
