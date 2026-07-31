<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import { inTauri } from "$lib/ipc";
  import { openSteamUrl, steamPropertiesUrl } from "$lib/util";
  import { SteamLogo } from "phosphor-svelte";

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
    class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50 active:scale-95"
  >
    <SteamLogo size={14} /> Open in Steam
  </button>
{/if}
