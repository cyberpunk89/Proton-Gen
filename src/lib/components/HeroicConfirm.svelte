<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { ipc } from "$lib/ipc";
  import Dialog from "./Dialog.svelte";
  import { WarningCircle } from "phosphor-svelte";

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

  /**
   * Heroic loads a game's settings into memory when it starts, and on exit (or
   * on launching the game) flushes that in-memory copy straight back over
   * `GamesConfig/<app_name>.json` — including whatever protongen just wrote,
   * which Heroic never re-read. The subtitle below always warned about this,
   * but as prose it's easy to skim past; check for real whenever the dialog
   * opens so the warning only shows — loudly — when it actually applies.
   */
  let heroicRunning = $state(false);
  $effect(() => {
    if (!app.heroicConfirmOpen) return;
    heroicRunning = false;
    ipc.heroicRunning().then((v) => (heroicRunning = v));
  });
</script>

<Dialog
  bind:open={app.heroicConfirmOpen}
  title="Apply to Heroic?"
  subtitle="Close Heroic first — it may overwrite these changes when it exits."
>
  <div class="space-y-4">
    {#if heroicRunning}
      <div
        class="flex items-start gap-2 rounded-xl border px-3 py-2.5 text-xs"
        style="border-color: color-mix(in srgb, var(--red) 35%, transparent); background: color-mix(in srgb, var(--red) 8%, transparent)"
        role="alert"
      >
        <WarningCircle size={16} weight="fill" class="mt-0.5 shrink-0 text-red" />
        <span class="text-subtext">
          <span class="font-medium text-red">Heroic is running.</span>
          Quit it before applying — Heroic caches this game's settings in memory and can
          overwrite what protongen writes the moment it exits or launches the game.
        </span>
      </div>
    {/if}
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
