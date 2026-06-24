<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { fly } from "svelte/transition";
  import { clickOutside, autofocus } from "$lib/actions";
  import { GameController, MagnifyingGlass, CaretUpDown, Check } from "phosphor-svelte";
  import type { GameDto } from "$lib/types";

  let open = $state(false);
  let query = $state("");

  let filtered = $derived(
    app.games.filter((g) =>
      g.name.toLowerCase().includes(query.trim().toLowerCase()),
    ),
  );

  // Lazily pull cover thumbnails for the games currently in view (capped so a
  // huge library doesn't trigger hundreds of CDN lookups at once).
  $effect(() => {
    if (!open) return;
    for (const g of filtered.slice(0, 100)) {
      app.requestArt(g.app_id, g.source, "portrait");
    }
  });

  function choose(g: GameDto | null) {
    app.selectGame(g);
    open = false;
    query = "";
  }
</script>

<div class="relative" use:clickOutside={() => (open = false)}>
  <button
    type="button"
    onclick={() => (open = !open)}
    class="flex w-full items-center gap-3 rounded-xl border border-border bg-surface-2 px-3.5 py-3 text-left transition hover:border-accent/60"
  >
    <span
      class="grid size-9 shrink-0 place-items-center rounded-lg"
      style="background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent)"
    >
      <GameController size={20} weight="fill" />
    </span>
    <span class="min-w-0 flex-1">
      <span class="block text-[11px] uppercase tracking-wider text-muted">Game</span>
      <span class="block truncate text-[15px] font-medium text-text">
        {app.selectedGameName ?? "Select a game or shortcut"}
      </span>
    </span>
    <CaretUpDown size={18} class="shrink-0 text-muted" />
  </button>

  {#if open}
    <div
      transition:fly={{ y: -4, duration: 120 }}
      class="popover absolute left-0 right-0 top-full z-50 mt-2 overflow-hidden p-0"
    >
      <div class="relative border-b border-border p-2">
        <MagnifyingGlass
          size={15}
          class="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-muted"
        />
        <input
          use:autofocus
          bind:value={query}
          placeholder="Filter {app.games.length} games…"
          class="w-full rounded-lg bg-surface-2 py-2 pl-8 pr-3 text-sm text-text outline-none"
        />
      </div>
      <div class="max-h-[320px] overflow-y-auto p-1.5">
        <button
          onclick={() => choose(null)}
          class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-sm text-subtext hover:bg-surface-2"
        >
          {#if app.selectedAppId == null}<Check size={14} class="text-accent" />{:else}<span
              class="size-3.5"
            ></span>{/if}
          No game — generic command
        </button>
        {#each filtered as g (g.app_id + g.source)}
          {@const art = app.artFor(g.app_id, g.source, "portrait")}
          <button
            onclick={() => choose(g)}
            class="flex w-full items-center gap-2.5 rounded-lg px-2 py-1.5 text-left hover:bg-surface-2 {app.selectedAppId ===
            g.app_id
              ? 'bg-surface-2'
              : ''}"
          >
            <span
              class="h-[40px] w-7 shrink-0 overflow-hidden rounded-md bg-surface-2 ring-1 ring-border/60"
            >
              {#if art}
                <img src={art} alt="" loading="lazy" class="h-full w-full object-cover" />
              {:else}
                <span class="grid h-full w-full place-items-center text-muted">
                  <GameController size={13} weight="fill" />
                </span>
              {/if}
            </span>
            <span class="min-w-0 flex-1 truncate text-sm text-text">{g.name}</span>
            {#if g.source === "non-steam"}
              <span
                class="shrink-0 rounded-full px-2 py-0.5 text-[10px]"
                style="background: color-mix(in srgb, var(--mauve) 18%, transparent); color: var(--mauve)"
                >shortcut</span
              >
            {/if}
            {#if app.selectedAppId === g.app_id}
              <Check size={14} class="shrink-0 text-accent" />
            {/if}
          </button>
        {:else}
          <p class="px-3 py-6 text-center text-sm text-muted">No matches.</p>
        {/each}
      </div>
    </div>
  {/if}
</div>
