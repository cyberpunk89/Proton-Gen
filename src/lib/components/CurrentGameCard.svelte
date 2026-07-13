<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { GameController, Terminal, ArrowLeft } from "phosphor-svelte";

  let game = $derived(
    app.selectedAppId == null
      ? null
      : app.games.find((g) => g.app_id === app.selectedAppId) ?? null,
  );

  $effect(() => {
    if (game) app.requestArt(game.app_id, game.source, "portrait");
  });

  let art = $derived(
    game ? app.artFor(game.app_id, game.source, "portrait") : undefined,
  );
</script>

<button
  onclick={() => app.backToLibrary()}
  title="Back to library"
  class="group flex w-full items-center gap-3 rounded-xl border border-border bg-surface-2 px-3 py-2.5 text-left transition hover:border-accent/60"
>
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
  </span>

  <span
    class="flex shrink-0 items-center gap-1 text-[11px] text-muted transition group-hover:text-accent"
  >
    <ArrowLeft size={13} weight="bold" /> Change
  </span>
</button>
