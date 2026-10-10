<script lang="ts" module>
  /** The staggered entrance plays once per session, not on every return from
   *  the builder — by the tenth time it is just a delay. */
  let entrancePlayed = false;
</script>

<script lang="ts">
  import { app, type LibrarySort } from "$lib/state.svelte";
  import { autofocus, focusByName, focusTarget } from "$lib/actions";
  import GameTile from "./GameTile.svelte";
  import SelectField from "./SelectField.svelte";
  import { toast } from "$lib/toast.svelte";
  import type { GameDto } from "$lib/types";
  import { groupGames } from "$lib/util";
  import { fuzzy } from "$lib/fuzzy";
  import { slide } from "$lib/motion.svelte";
  import {
    GameController,
    MagnifyingGlass,
    Terminal,
    CheckCircle,
    WarningCircle,
    CircleDashed,
    Circle,
    CaretRight,
    CheckSquare,
  } from "phosphor-svelte";

  let query = $state("");
  /** Filters are deliberately local, not persisted: a filter you forgot you set
   *  and that survives a restart looks like a missing library. */
  let tunedOnly = $state(false);

  // Cleared after the stagger finishes, so tiles that arrive later (a filter,
  // a rescan) appear immediately rather than replaying the entrance.
  let entering = $state(!entrancePlayed);
  $effect(() => {
    if (!entering) return;
    entrancePlayed = true;
    const t = setTimeout(() => (entering = false), 900);
    return () => clearTimeout(t);
  });
  let favoritesOnly = $state(false);

  /** Wraps both shelves: tile lookup and column measuring go through it, so
   *  either grid can be the one that exists. */
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
   * per comparison. The comparator below runs O(n log n) times on every
   * keystroke.
   */
  let lowerNames = $derived.by(() => {
    const m = new Map<number, string>();
    for (const g of app.games) m.set(g.app_id, g.name.toLowerCase());
    return m;
  });
  const lower = (g: GameDto) => lowerNames.get(g.app_id) ?? g.name.toLowerCase();

  /**
   * Fuzzy relevance per matching game, or null with no query — "eldn" finds
   * ELDEN RING. A number also finds a game by the start of its appid, ranked
   * below every name match.
   */
  let scores = $derived.by(() => {
    const q = query.trim();
    if (!q) return null;
    const digits = /^\d+$/.test(q);
    const m = new Map<number, number>();
    for (const g of app.games) {
      const hit = fuzzy(g.name, q);
      if (hit) m.set(g.app_id, hit.score);
      else if (digits && String(g.app_id).startsWith(q)) m.set(g.app_id, Number.EPSILON);
    }
    return m;
  });

  let matching = $derived.by(() => {
    return app.games.filter((g) => {
      if (scores && !scores.has(g.app_id)) return false;
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
   * While searching, relevance comes first: the best match for what was typed
   * leads. Otherwise favourites are a primary sort key, not a separate mode —
   * pinning is expected to hold under whatever sort is active. Every comparator
   * tiebreaks on the lowercased name so the grid can't jitter between renders.
   */
  let games = $derived.by(() => {
    const sort = app.librarySort;
    const byName = (a: GameDto, b: GameDto) => lower(a).localeCompare(lower(b));

    return [...matching].sort((a, b) => {
      if (scores) {
        const d = (scores.get(b.app_id) ?? 0) - (scores.get(a.app_id) ?? 0);
        if (d !== 0) return d;
      }
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
  let allGroups = $derived(groupGames(games));

  /**
   * Installed titles first; anything whose files are gone (see `on_disk` in
   * games.rs) goes on its own shelf below, collapsed by default — kept, not
   * hidden, since its saved tuning is still worth reaching. A merged tile is
   * installed if any of its launch paths is.
   */
  let installedGroups = $derived(allGroups.filter((g) => g.entries.some((e) => e.installed)));
  let missingGroups = $derived(allGroups.filter((g) => g.entries.every((e) => !e.installed)));

  const SHELF_KEY = "protongen.library.missingShelfOpen";
  let shelfOpen = $state(readShelf());
  function readShelf(): boolean {
    try {
      return localStorage.getItem(SHELF_KEY) === "1";
    } catch {
      return false;
    }
  }
  function toggleShelf() {
    shelfOpen = !shelfOpen;
    try {
      localStorage.setItem(SHELF_KEY, shelfOpen ? "1" : "0");
    } catch {
      // Private window / blocked storage: the shelf just won't remember.
    }
  }
  /** A search opens the shelf for its matches without changing the saved
   *  preference — a result you can't see looks like no result. */
  let shelfShown = $derived(shelfOpen || (scores != null && missingGroups.length > 0));

  /** Every tile the keyboard can reach, in index order: the roving index runs
   *  straight on from the installed grid into the shelf. */
  let groups = $derived(shelfShown ? [...installedGroups, ...missingGroups] : installedGroups);

  /** The shelf a tile index sits on, as [start, length]. */
  function sectionOf(i: number): [number, number] {
    const n = installedGroups.length;
    return i < n ? [0, n] : [n, missingGroups.length];
  }
  /** Tiles in the whole library, for the filter placeholder — counting raw
   *  entries said "5 games" over 4 tiles when one title is on two stores. */
  let tileCount = $derived(groupGames(app.games).length);

  // ---- batch apply (Select mode) ----
  let batchPreset = $state("");
  let presetOptions = $derived(app.store.presets.map((p) => ({ value: p.name, label: p.name })));
  $effect(() => {
    if (app.selectMode && !presetOptions.some((o) => o.value === batchPreset)) {
      batchPreset = presetOptions[0]?.value ?? "";
    }
  });

  /** Everything visible, including the Not-installed shelf when it is open —
   *  a preset written to a game that isn't on disk right now is still kept
   *  (and applies when it comes back), so skipping it surprised users. */
  function selectAllShown() {
    for (const g of groups) app.selectedForBatch.add(g.entries[0].app_id);
  }

  function clearFilters() {
    query = "";
    tunedOnly = false;
    favoritesOnly = false;
  }

  function applyBatch() {
    const ids = [...app.selectedForBatch];
    if (!batchPreset || ids.length === 0) return;
    const { count, undo } = app.applyPresetToGames(batchPreset, ids);
    app.setSelectMode(false);
    toast.success(`Applied “${batchPreset}” to ${count} game${count === 1 ? "" : "s"}`, {
      action: { label: "Undo", onClick: undo },
    });
  }

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
      const g = grid!.querySelector<HTMLElement>("[data-grid]");
      if (!g) return;
      const n = getComputedStyle(g).gridTemplateColumns.split(" ").filter(Boolean).length;
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
    tunedOnly;
    favoritesOnly;
    activeIndex = 0;
    keyboardMoved = false;
  });

  // A rescan can shrink the library under an unchanged query; an index past
  // the end leaves no tile with tabindex=0, dropping the grid from Tab order.
  $effect(() => {
    if (activeIndex >= groups.length) activeIndex = Math.max(0, groups.length - 1);
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
      case "ArrowDown": {
        e.preventDefault();
        // Rows are counted per shelf: the shelf starts a fresh row, so from the
        // installed grid's last row, Down lands in the same column below.
        const [start, len] = sectionOf(activeIndex);
        const r = activeIndex - start;
        if (r + cols < len) move(cols);
        else if (start + len < groups.length) {
          activeIndex = start + len + Math.min(r % cols, groups.length - start - len - 1);
          keyboardMoved = true;
        }
        break;
      }
      case "ArrowUp": {
        e.preventDefault();
        const [start] = sectionOf(activeIndex);
        const r = activeIndex - start;
        if (r >= cols) move(-cols);
        else if (start > 0) {
          // Into the installed grid's last row, same column where it has one.
          const lastRow = Math.floor((start - 1) / cols) * cols;
          activeIndex = Math.min(lastRow + (r % cols), start - 1);
          keyboardMoved = true;
        }
        // From the top row, step out to the filter box so `/` → Down → arrows →
        // Up is one continuous loop rather than a dead end.
        else focusByName("library-filter");
        break;
      }
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

  const GRID = "grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6";

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
          placeholder="Filter {tileCount} games…"
          class="w-56 rounded-xl border border-border bg-surface-2 py-2 pl-9 pr-3 text-sm text-text outline-none focus:border-accent"
        />
      </div>
      <button
        onclick={() => app.setSelectMode(!app.selectMode)}
        aria-pressed={app.selectMode}
        title="Select several games to apply a preset to all of them"
        class="inline-flex items-center gap-2 rounded-xl border px-3 py-2 text-sm transition {app.selectMode
          ? 'border-accent bg-accent/15 text-text'
          : 'border-border bg-surface-2/50 text-subtext hover:border-accent/60'}"
      >
        <CheckSquare size={15} /> Select
      </button>
      <button
        onclick={() => app.openGeneric()}
        class="inline-flex items-center gap-2 rounded-xl border border-border bg-surface-2/50 px-3 py-2 text-sm text-subtext transition hover:border-accent/60"
      >
        <Terminal size={15} /> Generic command
      </button>
    </div>
  </div>

  {#if app.selectMode}
    <!-- Batch bar: one preset onto every selected game's saved tuning. -->
    <div
      class="sticky top-0 z-50 flex flex-wrap items-center gap-2 rounded-xl border border-accent/40 bg-surface-solid px-3 py-2 text-sm"
      role="region"
      aria-label="Apply a preset to selected games"
    >
      <span class="text-subtext" aria-live="polite">{app.selectedForBatch.size} selected</span>
      <button onclick={selectAllShown} class="text-xs text-accent hover:opacity-80">Select all shown</button>
      {#if app.selectedForBatch.size}
        <button onclick={() => app.selectedForBatch.clear()} class="text-xs text-muted hover:text-text">Clear</button>
      {/if}
      <span class="ml-auto flex items-center gap-2">
        {#if presetOptions.length}
          <span class="text-xs text-muted">Preset</span>
          <SelectField label="Preset to apply" value={batchPreset} options={presetOptions} onValueChange={(v) => (batchPreset = v)} width="w-44" />
          <button
            onclick={applyBatch}
            disabled={!app.selectedForBatch.size || !batchPreset}
            class="rounded-lg px-3 py-1.5 text-xs font-medium transition active:scale-95 disabled:opacity-40"
            style="background: var(--accent); color: var(--on-accent)">Apply to selected</button
          >
        {:else}
          <span class="text-xs text-muted">Save a preset in the builder first.</span>
        {/if}
        <button onclick={() => app.setSelectMode(false)} class="text-xs text-muted hover:text-text">Done</button>
      </span>
    </div>
  {/if}

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
    <div class="animate-fade-up flex flex-col items-center gap-2 py-24 text-center">
      <span
        class="mb-2 grid size-16 place-items-center rounded-2xl text-accent ring-1 ring-accent/25"
        style="background: radial-gradient(circle at 50% 30%, color-mix(in srgb, var(--accent) 22%, transparent), color-mix(in srgb, var(--accent) 6%, transparent))"
      >
        <GameController size={30} weight="duotone" />
      </span>
      <p class="text-sm text-muted">
        {app.games.length === 0
          ? "No games or shortcuts found."
          : "No games match those filters."}
      </p>
      {#if app.games.length === 0}
        <p class="text-xs text-muted">
          If you have games installed, check
          <button
            class="underline underline-offset-2 hover:text-text"
            onclick={() => app.openSettings("paths")}>Settings → Paths</button
          > for extra Steam library folders.
        </p>
      {:else}
        <button
          onclick={clearFilters}
          class="mt-1 rounded-lg border border-border px-3 py-1.5 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
        >
          Clear filters
        </button>
      {/if}
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
      class="flex flex-col gap-6 {entering ? 'tiles-entering' : ''}"
    >
      {#if installedGroups.length}
        <div data-grid class={GRID}>
          {#each installedGroups as grp, i (grp.key)}
            <GameTile
              entries={grp.entries}
              index={i}
              active={i === activeIndex}
              onactivate={(n) => (activeIndex = n)}
            />
          {/each}
        </div>
      {/if}

      {#if missingGroups.length}
        <section class="flex flex-col gap-3" aria-label="Not installed">
          <button
            onclick={toggleShelf}
            aria-expanded={shelfShown}
            class="group/shelf flex items-center gap-2 self-start rounded-lg py-1 pr-2 text-sm text-muted transition hover:text-text"
          >
            <CaretRight
              size={13}
              weight="bold"
              class="transition-transform duration-200 {shelfShown ? 'rotate-90' : ''}"
            />
            <span class="font-medium">Not installed</span>
            <span class="rounded-full bg-surface-2 px-1.5 py-px text-[11px] tabular-nums">{missingGroups.length}</span>
            <span class="hidden text-xs text-muted/80 group-hover/shelf:inline">
              — missing from disk or not fully installed. Saved settings are kept.
            </span>
          </button>
          {#if shelfShown}
            <div data-grid class={GRID} transition:slide={{ duration: 180 }}>
              {#each missingGroups as grp, i (grp.key)}
                {@const n = installedGroups.length + i}
                <GameTile
                  entries={grp.entries}
                  index={n}
                  active={n === activeIndex}
                  onactivate={(k) => (activeIndex = k)}
                />
              {/each}
            </div>
          {/if}
        </section>
      {/if}
    </div>
  {/if}
</div>
