<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import { inTauri } from "$lib/ipc";
  import { openSteamUrl, steamPropertiesUrl } from "$lib/util";
  import { SteamLogo } from "phosphor-svelte";

  /**
   * `collapsible` drops the text label when the surrounding container is narrow,
   * leaving the glyph. Opt-in rather than automatic because the popover and the
   * game panel have room for the words, while the command bar does not — and a
   * portalled popover has no container ancestor to query anyway.
   */
  let { collapsible = false }: { collapsible?: boolean } = $props();

  /** Renders nothing unless a deep link is meaningful — see `app.steamAppId`.
   *  A non-Steam shortcut in particular must never get this button: its appid is
   *  a synthetic shortcut id and `gameproperties` would open the wrong thing or
   *  nothing at all. */
  async function open() {
    const id = app.steamAppId;
    if (id == null) return;

    if (await openSteamUrl(steamPropertiesUrl(id))) return;

    // Two distinct failures worth telling apart: the dev browser has no handler
    // by design, whereas a failure in the real shell means something is wrong.
    toast.info(
      inTauri
        ? "Couldn't hand that link to Steam. Is Steam installed?"
        : "Steam deep links only work in the desktop app.",
    );
  }
</script>

{#if app.steamAppId != null}
  <button
    onclick={open}
    title="Open this game's Properties in Steam — Launch Options is on the General tab"
    aria-label="Open in Steam"
    class="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 py-1.5 text-xs text-subtext transition hover:border-accent/50 active:scale-95 {collapsible
      ? 'px-2 @2xl:px-2.5'
      : 'px-2.5'}"
  >
    <SteamLogo size={14} />
    <span class={collapsible ? "hidden @2xl:inline" : ""}>Open in Steam</span>
  </button>
{/if}
