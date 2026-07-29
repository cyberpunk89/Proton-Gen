<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { autofocus, focusTarget } from "$lib/actions";
  import { GameController, MagnifyingGlass, Terminal, Check } from "phosphor-svelte";

  let query = $state("");

  let filtered = $derived(
    app.games.filter((g) =>
      g.name.toLowerCase().includes(query.trim().toLowerCase()),
    ),
  );

  // Lazily pull cover art for the games in view (capped so a big library doesn't
  // fire hundreds of CDN lookups at once).
  $effect(() => {
    for (const g of filtered.slice(0, 300)) {
      app.requestArt(g.app_id, g.source, "portrait");
    }
  });

  // Games that already have saved settings — surfaced with a subtle "tuned" dot.
  function isTuned(appId: number): boolean {
    return app.store.game_memory[String(appId)] != null;
  }
</script>

<div class="mx-auto flex w-full max-w-6xl flex-col gap-5 px-6 py-6">
  <div class="flex flex-wrap items-end justify-between gap-4">
    <div>
      <h2 class="text-lg font-semibold text-text">Choose a game</h2>
      <p class="mt-0.5 text-[13px] text-muted">
        Pick a title to build its launch command — or start a generic one.
      </p>
    </div>

    <div class="flex items-center gap-2">
      <div class="relative">
        <MagnifyingGlass
          size={15}
          class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted"
        />
        <input
          use:autofocus
          use:focusTarget={"library-filter"}
          bind:value={query}
          aria-label="Filter games"
          placeholder="Filter {app.games.length} games…"
          class="w-56 rounded-xl border border-border bg-surface-2 py-2 pl-9 pr-3 text-sm text-text outline-none focus:border-accent"
        />
      </div>
      <button
        onclick={() => app.openGeneric()}
        class="inline-flex items-center gap-2 rounded-xl border border-border bg-surface-2/50 px-3 py-2 text-sm text-subtext transition hover:border-accent/60"
      >
        <Terminal size={15} /> Generic command
      </button>
    </div>
  </div>

  {#if filtered.length === 0}
    <div class="flex flex-col items-center gap-2 py-24 text-center">
      <GameController size={30} class="text-muted" />
      <p class="text-sm text-muted">
        {app.games.length === 0
          ? "No games or shortcuts found."
          : `No games match “${query.trim()}”.`}
      </p>
    </div>
  {:else}
    <div
      class="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
    >
      {#each filtered as g (g.app_id + g.source)}
        {@const art = app.artFor(g.app_id, g.source, "portrait")}
        {@const selected = app.selectedAppId === g.app_id}
        <button
          onclick={() => app.openGame(g)}
          class="group relative aspect-[2/3] overflow-hidden rounded-xl bg-surface-2 text-left ring-1 ring-border/60 transition duration-150 hover:-translate-y-1 hover:ring-2 hover:ring-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent {selected
            ? 'ring-2 ring-accent'
            : ''}"
        >
          {#if art}
            <img
              src={art}
              alt={g.name}
              loading="lazy"
              class="h-full w-full object-cover transition duration-300 group-hover:scale-[1.04]"
            />
          {:else}
            <span
              class="grid h-full w-full place-items-center text-muted transition group-hover:text-subtext"
            >
              <GameController size={34} weight="fill" />
            </span>
          {/if}

          <!-- Legibility gradient + title (always shown; covers placeholder tiles
               and gives Steam art a consistent caption). -->
          <span
            class="pointer-events-none absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/85 via-black/45 to-transparent px-2.5 pb-2 pt-8"
          >
            <span class="line-clamp-2 text-xs font-medium leading-snug text-white">
              {g.name}
            </span>
          </span>

          <!-- Badges -->
          <span class="absolute left-2 top-2 flex gap-1">
            {#if g.source === "non-steam"}
              <span
                class="rounded-full px-1.5 py-0.5 text-[10px] font-medium backdrop-blur-sm"
                style="background: color-mix(in srgb, var(--mauve) 75%, transparent); color: var(--on-accent)"
                >shortcut</span
              >
            {/if}
          </span>
          {#if isTuned(g.app_id)}
            <span
              class="absolute right-2 top-2 grid size-5 place-items-center rounded-full backdrop-blur-sm"
              style="background: color-mix(in srgb, var(--accent) 85%, transparent); color: var(--on-accent)"
              title="Has saved settings"
            >
              <Check size={12} weight="bold" />
            </span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>
