<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { formatLastPlayed, formatPlaytime } from "$lib/util";
  import { GameController, Terminal, ArrowLeft } from "phosphor-svelte";

  /** Also show playtime / last played under the name (Simple mode's header). */
  let { details = false }: { details?: boolean } = $props();

  let game = $derived(
    app.selectedAppId == null
      ? null
      : app.games.find((g) => g.app_id === app.selectedAppId) ?? null,
  );

  $effect(() => {
    void app.artEpoch; // re-ask after a refresh forgets failed art
    if (game) app.requestArt(game.app_id, game.source, "portrait");
  });

  let art = $derived(
    game ? app.artFor(game.app_id, game.source, "portrait") : undefined,
  );

  let stats = $derived.by(() => {
    if (!details || !game) return "";
    const played = formatPlaytime(game.playtime_minutes);
    const last = formatLastPlayed(game.last_played);
    return [played && `${played} played`, last && `last played ${last}`].filter(Boolean).join(" · ");
  });
</script>

<button
  onclick={() => app.backToLibrary()}
  title="Back to library"
  class="group relative isolate flex w-full items-center gap-3 overflow-hidden rounded-xl border border-border bg-surface-2 px-3 py-2.5 text-left transition hover:border-accent/60"
>
  {#if game && art}
    <!-- The cover, blown up and blurred into an ambient wash behind the card,
         fading out to the right so the text stays on a calm surface. -->
    <img
      src={art}
      alt=""
      aria-hidden="true"
      class="pointer-events-none absolute inset-0 -z-10 h-full w-full scale-150 object-cover opacity-30 blur-2xl saturate-150 transition-opacity duration-500 group-hover:opacity-45"
    />
    <span
      class="pointer-events-none absolute inset-0 -z-10"
      style="background: linear-gradient(90deg, transparent, var(--surface-2) 75%)"
    ></span>
  {/if}
  <span
    class="h-[42px] w-[30px] shrink-0 overflow-hidden rounded-md bg-mantle ring-1 ring-border/60"
  >
    {#if game && art}
      <img src={art} alt="" class="h-full w-full object-cover" />
    {:else}
      <span class="grid h-full w-full place-items-center text-muted">
        {#if game}
          <GameController size={15} weight="fill" />
        {:else}
          <Terminal size={15} />
        {/if}
      </span>
    {/if}
  </span>

  <span class="min-w-0 flex-1">
    <span class="block text-[11px] uppercase tracking-wider text-muted">Game</span>
    <span class="block truncate text-[14px] font-medium text-text">
      {app.selectedGameName ?? "Generic command"}
    </span>
    {#if stats}
      <span class="block truncate text-[11px] text-muted">{stats}</span>
    {/if}
  </span>

  <span
    class="flex shrink-0 items-center gap-1 text-[11px] text-muted transition group-hover:text-accent"
  >
    <ArrowLeft size={13} weight="bold" /> Change
  </span>
</button>
