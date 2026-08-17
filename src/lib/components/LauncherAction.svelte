<script lang="ts">
  import { app } from "$lib/state.svelte";
  import OpenInSteam from "./OpenInSteam.svelte";
  import { Export } from "phosphor-svelte";

  /**
   * The one "get this into your launcher" slot.
   *
   * Steam and Heroic want opposite things — Steam takes a launch string you
   * paste into its Properties dialog, Heroic takes structured per-game JSON we
   * write directly — but from where the user stands they are the same step, and
   * exactly one of them applies to the selected game. They used to be two
   * separately-gated controls in two different places (Steam pinned in the
   * command bar, Heroic buried in Game & runtime), which made the Heroic path
   * easy to miss entirely.
   *
   * `app.steamAppId` and `app.heroicId` are mutually exclusive by construction:
   * each returns null unless `selectedGame.source` matches. A non-Steam shortcut
   * gets neither, which is correct — its synthetic appid can't be deep-linked.
   *
   * This renders the *button only*. The confirm dialog lives once in App.svelte
   * (see `HeroicConfirm`), keyed off `app.heroicConfirmOpen`, because this
   * component is mounted at two call sites at once and both the command bar and
   * the Game & runtime panel unmount out from under it — a bits-ui modal that
   * gets unmounted while open never runs its teardown, and leaves
   * `body { pointer-events: none }` behind. That is the #63 failure mode, and it
   * bricks every click in the app.
   */
  let { collapsible = false }: { collapsible?: boolean } = $props();
</script>

{#if app.heroicId != null}
  <button
    onclick={() => (app.heroicConfirmOpen = true)}
    title="Write these environment variables and wrappers into this game's Heroic config"
    aria-label="Apply to Heroic"
    class="inline-flex shrink-0 items-center gap-1.5 rounded-lg bg-accent py-1.5 text-xs font-medium text-on-accent transition hover:opacity-90 active:scale-95 {collapsible
      ? 'px-2 @2xl:px-2.5'
      : 'px-2.5'}"
  >
    <Export size={14} />
    <span class={collapsible ? "hidden @2xl:inline" : ""}>Apply to Heroic</span>
  </button>
{:else}
  <OpenInSteam {collapsible} />
{/if}
