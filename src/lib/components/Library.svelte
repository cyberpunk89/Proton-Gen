<script lang="ts">
  import { app, type LibrarySort } from "$lib/state.svelte";
  import { autofocus, focusByName, focusTarget } from "$lib/actions";
  import GameTile from "./GameTile.svelte";
  import type { GameDto } from "$lib/types";
  import { groupGames } from "$lib/util";
  import {
    GameController,
    MagnifyingGlass,
    Terminal,
    CheckCircle,
    WarningCircle,
    CircleDashed,
    Circle,
  } from "phosphor-svelte";

  let query = $state("");
  /** Filters are deliberately local, not persisted: a filter you forgot you set
   *  and that survives a restart looks like a missing library. */
  let installedOnly = $state(false);
  let tunedOnly = $state(false);
  let favoritesOnly = $state(false);

  let grid = $state<HTMLDivElement | null>(null);
  let cols = $state(1);
  let activeIndex = $state(0);
  /** Only steal focus when the keyboard moved the index — otherwise clicking a
   *  tile would yank focus back on every re-render. */
  let keyboardMoved = $state(false);

  const SORTS: { id: LibrarySort; label: string }[] = [
    { id: "recent", label: "Recently played" },
    { id: "alpha", label: "Alphabetical" },
    { id: "tuned", label: "Tuned first" },
  ];

  function isTuned(appId: number): boolean {
    return app.store.game_memory[String(appId)] != null;
  }

  /**
   * Lowercased names, keyed by appid — built once per library change rather than
   * per comparison. The filter below and the comparator underneath it both need
   * them, and the comparator runs O(n log n) times on every keystroke.
   */
  let lowerNames = $derived.by(() => {
    const m = new Map<number, string>();
    for (const g of app.games) m.set(g.app_id, g.name.toLowerCase());
    return m;
  });
  const lower = (g: GameDto) => lowerNames.get(g.app_id) ?? g.name.toLowerCase();

  let matching = $derived.by(() => {
    // Hoisted: this was re-normalized once per game.
    const needle = query.trim().toLowerCase();
    return app.games.filter((g) => {
      if (needle && !lower(g).includes(needle)) return false;
      if (installedOnly && !g.installed) return false;
      if (tunedOnly && !isTuned(g.app_id)) return false;
      if (favoritesOnly && !app.isFavorite(g.app_id)) return false;
      return true;
    });
  });

  /** True when nothing in the library has a play timestamp — Flatpak Steam is
   *  deliberately excluded from the localconfig.vdf scan, and a fresh install has
   *  none either. Saying so beats labelling alphabetical order "Recently played". */
  let noTimestamps = $derived(app.games.every((g) => g.last_played == null));

  /**
   * Re-sorted here rather than relying on the order `games.rs` happens to emit,
   * so DTO ordering never becomes load-bearing.
   *
   * Favourites are a primary sort key, not a separate mode — pinning is expected
   * to hold under whatever sort is active. Every comparator tiebreaks on the
   * lowercased name so the grid can't jitter between renders.
   */
  let games = $derived.by(() => {
    const sort = app.librarySort;
    const byName = (a: GameDto, b: GameDto) => lower(a).localeCompare(lower(b));

    return [...matching].sort((a, b) => {
      const favA = app.isFavorite(a.app_id) ? 0 : 1;
      const favB = app.isFavorite(b.app_id) ? 0 : 1;
      if (favA !== favB) return favA - favB;

      if (sort === "recent") {
        // Never-played sinks rather than sorting as epoch 0 among real dates.
        const ta = a.last_played ?? -1;
        const tb = b.last_played ?? -1;
        if (ta !== tb) return tb - ta;
      } else if (sort === "tuned") {
        const tA = isTuned(a.app_id) ? 0 : 1;
        const tB = isTuned(b.app_id) ? 0 : 1;
        if (tA !== tB) return tA - tB;
      }
      return byName(a, b);
    });
  });

  /**
   * Games folded into one tile per group — the "same title on Steam and
   * sideloaded in Heroic" case. `games`' sort order is preserved (a group lands
   * at its first member's position), so favourites/recent/alphabetical still
   * governs where a merged tile sits; `GameTile` picks which underlying entry
   * to open, directly if there's only one.
   */
  let groups = $derived(groupGames(games));

  /** Any status badge on screen at all — no point explaining glyphs the user
   *  cannot see, which is what a fresh install would get. */
  let showLegend = $derived(
    games.some(
      (g) =>
        g.source === "steam" &&
        (isTuned(g.app_id) || (app.launchOptions[String(g.app_id)] ?? "").trim() !== ""),
    ),
  );

  const LEGEND = [
    { icon: CheckCircle, weight: "fill" as const, colour: "var(--green)", label: "Applied" },
    { icon: WarningCircle, weight: "fill" as const, colour: "var(--peach)", label: "Not pasted" },
    { icon: CircleDashed, weight: "bold" as const, colour: "var(--accent)", label: "Saved only" },
    { icon: Circle, weight: "bold" as const, colour: "var(--muted)", label: "Set outside protongen" },
  ];

  // Column count is measured, never derived from the grid-cols-* classes — those
  // drift the moment someone edits them, and the arrow maths would drift with it.
  $effect(() => {
    if (!grid) return;
    const measure = () => {
      const n = getComputedStyle(grid!).gridTemplateColumns.split(" ").filter(Boolean).length;
      cols = Math.max(1, n);
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(grid);
    return () => ro.disconnect();
  });

  // A shrinking result set would otherwise leave activeIndex dangling past the
  // end. Reset rather than clamp: after a new query, position 0 is what the user
  // means.
  $effect(() => {
    query;
    app.librarySort;
    installedOnly;
    tunedOnly;
    favoritesOnly;
    activeIndex = 0;
    keyboardMoved = false;
  });

  $effect(() => {
    if (!keyboardMoved || !grid) return;
    const el = grid.querySelector<HTMLElement>(`[data-tile="${activeIndex}"]`);
    el?.focus();
    el?.scrollIntoView({ block: "nearest" });
    keyboardMoved = false;
  });

  function move(delta: number) {
    if (!groups.length) return;
    const next = activeIndex + delta;
    if (next < 0 || next >= groups.length) return;
    activeIndex = next;
    keyboardMoved = true;
  }

  function onKeydown(e: KeyboardEvent) {
    if (!groups.length) return;
    switch (e.key) {
      case "ArrowRight":
        e.preventDefault();
        move(1); // wraps across the row boundary by design
        break;
      case "ArrowLeft":
        e.preventDefault();
        move(-1);
        break;
      case "ArrowDown":
        e.preventDefault();
        move(cols);
        break;
      case "ArrowUp":
        e.preventDefault();
        // From the top row, step out to the filter box so `/` → Down → arrows →
        // Up is one continuous loop rather than a dead end.
        if (activeIndex < cols) focusByName("library-filter");
        else move(-cols);
        break;
      case "Home":
        e.preventDefault();
        activeIndex = 0;
        keyboardMoved = true;
        break;
      case "End":
        e.preventDefault();
        activeIndex = groups.length - 1;
        keyboardMoved = true;
        break;
    }
  }

  /** Down from the filter box drops into the grid. */
  function onFilterKeydown(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" || !groups.length) return;
    e.preventDefault();
    activeIndex = 0;
    keyboardMoved = true;
  }

  const chip = (on: boolean) =>
    `rounded-full border px-2.5 py-1 text-xs transition ${
      on
        ? "border-accent/60 bg-accent/15 text-text"
        : "border-border bg-surface-2/50 text-muted hover:text-subtext"
    }`;
</script>

<div class="mx-auto flex w-full max-w-6xl flex-col gap-4 px-6 py-6">
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
          onkeydown={onFilterKeydown}
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

  <!-- Sort + filters -->
  <div class="flex flex-wrap items-center gap-x-4 gap-y-2">
    <div
      class="inline-flex rounded-xl border border-border bg-surface-2/60 p-0.5"
      role="group"
      aria-label="Sort library"
    >
      {#each SORTS as s (s.id)}
        <button
          onclick={() => app.setLibrarySort(s.id)}
          aria-pressed={app.librarySort === s.id}
          class="rounded-lg px-2.5 py-1 text-xs font-medium transition {app.librarySort === s.id
            ? 'bg-accent text-on-accent'
            : 'text-muted hover:text-subtext'}"
        >
          {s.label}
        </button>
      {/each}
    </div>

    <div class="flex flex-wrap items-center gap-1.5">
      <button
        onclick={() => (installedOnly = !installedOnly)}
        aria-pressed={installedOnly}
        class={chip(installedOnly)}>Installed</button
      >
      <button onclick={() => (tunedOnly = !tunedOnly)} aria-pressed={tunedOnly} class={chip(tunedOnly)}
        >Tuned</button
      >
      <button
        onclick={() => (favoritesOnly = !favoritesOnly)}
        aria-pressed={favoritesOnly}
        class={chip(favoritesOnly)}>Favourites</button
      >
    </div>

    {#if showLegend}
      <div class="ml-auto flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted">
        {#each LEGEND as l (l.label)}
          {@const Icon = l.icon}
          <span class="inline-flex items-center gap-1">
            <Icon size={12} weight={l.weight} style="color: {l.colour}" />
            {l.label}
          </span>
        {/each}
      </div>
    {/if}
  </div>

  {#if app.librarySort === "recent" && noTimestamps}
    <!-- Better than silently showing alphabetical order under a "Recently
         played" label: steam.rs excludes Flatpak Steam from the localconfig scan,
         so this is a real configuration, not a bug. -->
    <p class="text-xs text-muted">
      No play times found — Steam hasn't recorded any, or it's installed via
      Flatpak. Showing alphabetical order.
    </p>
  {/if}

  {#if games.length === 0}
    <div class="flex flex-col items-center gap-2 py-24 text-center">
      <GameController size={30} class="text-muted" />
      <p class="text-sm text-muted">
        {app.games.length === 0
          ? "No games or shortcuts found."
          : "No games match those filters."}
      </p>
    </div>
  {:else}
    <!--
      Roving tabindex: the grid is one tab stop, not one per game. On a real
      library that is the difference between arrowing across the screen and
      hundreds of Tab presses. Arrow handling lives here rather than on
      svelte:window so it is correctly scoped and needs no typing guard.
    -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      bind:this={grid}
      onkeydown={onKeydown}
      role="group"
      aria-label="Game library"
      class="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6"
    >
      {#each groups as grp, i (grp.key)}
        <GameTile
          entries={grp.entries}
          index={i}
          active={i === activeIndex}
          onactivate={(n) => (activeIndex = n)}
        />
      {/each}
    </div>
  {/if}
</div>
