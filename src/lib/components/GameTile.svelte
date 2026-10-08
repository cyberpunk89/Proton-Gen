<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { art as artStore } from "$lib/art.svelte";
  import { lookups } from "$lib/lookups.svelte";
  import { inView, clickOutside, autofocus } from "$lib/actions";
  import { keys } from "$lib/keys.svelte";
  import { tierColor, formatPlaytime, formatLastPlayed, sourceBadge, sourceColor, sourceLabel } from "$lib/util";
  import type { GameDto } from "$lib/types";
  import {
    GameController,
    CheckCircle,
    WarningCircle,
    CircleDashed,
    Circle,
    Star,
  } from "phosphor-svelte";

  /**
   * One library tile.
   *
   * ## Why this is a div and not a button
   *
   * The tile used to be a single `<button>`. A favourite star nested inside it
   * would be invalid HTML (and drag/click targets inside a button are
   * unreliable), so the open action is now an absolutely-positioned full-bleed
   * button with the star and badges as z-ordered *siblings* rather than
   * children. Everything that needs to sit on top of the open target gets
   * `relative z-10`.
   *
   * ## `entries`, not `game`
   *
   * A tile can represent more than one underlying `GameDto` — the same title
   * installed both via Steam and sideloaded in Heroic, which `groupGames`
   * (`$lib/util`) folds into one tile rather than showing twice. `primary`
   * (`entries[0]`, sorted Steam > non-Steam > Heroic) drives everything a
   * single game already had — favourite, ProtonDB tier, the sync-status badge
   * — since those are the properties of "the game", not of one launch path.
   * Only the open action and the source badges look at every entry.
   */

  let {
    entries,
    index,
    active,
    onactivate,
  }: {
    entries: GameDto[];
    /** Position in the filtered list, for the grid's roving tabindex. */
    index: number;
    /** True when this tile owns the grid's single tab stop. */
    active: boolean;
    /** Called when the tile takes focus, so the grid can follow. */
    onactivate: (index: number) => void;
  } = $props();

  let game = $derived(entries[0]);

  // `inView` fires once; the effect re-asks after a refresh forgets failed or
  // queued art (`artEpoch`), which a one-shot observer never would.
  let seen = $state(false);
  $effect(() => {
    if (!seen) return;
    void artStore.epoch;
    entries.forEach((e) => artStore.request(e.app_id, e.source, "portrait"));
  });
  let multi = $derived(entries.length > 1);
  /** Every launch path's files are gone (or the launcher says so): the tile
   *  sits on the library's "Not installed" shelf, drawn dimmed. */
  let missing = $derived(entries.every((e) => !e.installed));

  /**
   * The "open via" choice for a multi-source tile, shown as a scrim over the
   * tile's own bounds rather than a floating popover.
   *
   * This used to be a bits-ui Popover anchored to the full-bleed open button.
   * That worked, but tiles sit shoulder to shoulder in a dense grid *and*
   * translate up on hover (`hover:-translate-y-1` below), so the floating
   * popover's position — computed from the trigger's box before that
   * transform — could land a source button over the neighbouring tile instead
   * of this one. An overlay pinned to `inset-0` of the tile itself has nothing
   * to collide with: it can never spill onto another card, so there is no
   * wrong tile left to misclick.
   */
  let pickerOpen = $state(false);
  let triggerEl = $state<HTMLButtonElement | null>(null);
  $effect(() => {
    if (!pickerOpen) return;
    keys.pushOverlay();
    return () => keys.popOverlay();
  });
  /** Cancelling (Escape, click outside, the Cancel button) restores focus to
   *  the trigger, same as bits-ui's Popover did — otherwise focus is left on
   *  a button that just got removed from the DOM. */
  function closePicker() {
    pickerOpen = false;
    triggerEl?.focus();
  }

  /** First entry with art, so a Heroic sideload's cover can stand in for a
   *  Steam listing that has none cached yet (or vice-versa). */
  let artLoaded = $state(false);
  /** The image itself failed to load (the protocol 404'd, or it doesn't
   *  decode): show the placeholder rather than an invisible <img>. */
  let artFailed = $state(false);
  let art = $derived.by(() => {
    for (const e of entries) {
      const a = artStore.for(e.app_id, e.source, "portrait");
      if (a) return a;
    }
    return undefined;
  });
  $effect(() => {
    void art;
    artLoaded = false;
    artFailed = false;
  });

  let selected = $derived(entries.some((e) => app.selectedAppId === e.app_id));
  let favorite = $derived(app.isFavorite(game.app_id));

  /** Cached only — see `tier` below. */
  let tier = $derived(lookups.tierFor(game.app_id));

  /** Every source represented in this tile besides Steam's (which gets no
   *  badge of its own, same as a single-source Steam tile today). */
  let extraSources = $derived([...new Set(entries.map((e) => e.source))].filter((s) => s !== "steam"));

  /**
   * The single status indicator. "Has saved settings" is a precondition for
   * every diff state, so a separate tuned dot would always co-occur with this
   * and say the same thing twice.
   *
   * Non-Steam shortcuts are gated out entirely: they have no launch options, so
   * an absent entry means *untracked*, not *not-applied*, and a badge here would
   * simply lie on every shortcut.
   */
  let status = $derived.by(() => {
    if (game.source !== "steam") return null;

    const saved = app.store.game_memory[String(game.app_id)] != null;
    const steamHas = (app.launchOptions[String(game.app_id)] ?? "").trim() !== "";

    if (!saved) {
      return steamHas
        ? {
            icon: Circle,
            colour: "var(--muted)",
            weight: "bold" as const,
            label: "Steam has launch options protongen didn't set",
          }
        : null;
    }

    switch (app.launchStatuses[String(game.app_id)]) {
      case "in-sync":
        return {
          icon: CheckCircle,
          colour: "var(--green)",
          weight: "fill" as const,
          label: "Applied in Steam",
        };
      case "drifted":
        return {
          icon: WarningCircle,
          colour: "var(--peach)",
          weight: "fill" as const,
          label: "Changes not pasted",
        };
      case "not-applied":
        return {
          icon: CircleDashed,
          colour: "var(--accent)",
          weight: "bold" as const,
          label: "Saved — not in Steam yet",
        };
      // "umu": a umu config says nothing about Steam's launch options, so there
      // is no honest verdict to show. Undefined means not computed yet.
      default:
        return null;
    }
  });

  /** "12 h · 3 days ago". Only Steam records these, so a merged tile takes the
   *  best of its entries rather than whatever `primary` happens to be. */
  let playLine = $derived.by(() => {
    const minutes = Math.max(0, ...entries.map((e) => e.playtime_minutes ?? 0));
    const last = Math.max(0, ...entries.map((e) => e.last_played ?? 0));
    return [formatPlaytime(minutes), formatLastPlayed(last)].filter(Boolean).join(" · ");
  });

  /** Hover/accessible text for the open target — folds in the multi-source
   *  hint so "there's a choice here" isn't badge-only information. */
  let openDescription = $derived(
    multi ? `choose ${entries.map((e) => sourceLabel(e.source)).join(" or ")} to open` : "",
  );
  let titleText = $derived(
    [game.name, missing && "Not installed", playLine, status?.label, openDescription]
      .filter(Boolean)
      .join(" — "),
  );
  let srText = $derived(
    `${game.name}${missing ? ", not installed" : ""}${playLine ? `, ${playLine}` : ""}${status ? `, ${status.label}` : ""}${
      favorite ? ", favourite" : ""
    }${openDescription ? `, ${openDescription}` : ""}`,
  );
</script>

<!--
  Hover must stay compositor-only: WebKitGTK repaints on every frame anything
  else that animates. So the wrapper only translates (on a layer that exists
  before the first hover), the ring + glow are painted once on their own span
  and only fade in, and nothing over the zooming art uses backdrop-filter.
  The glow is a sibling after the card, not inside it: the card's
  overflow-hidden would clip it, and painting on top lets its 2px accent ring
  cover the card's own 1px one.
-->
<div
  data-tile-root
  style="--i: {Math.min(index, 14)}"
  class="group/tile relative aspect-[2/3] transition-transform duration-200 will-change-transform hover:-translate-y-1"
>
  <div
    class="relative h-full w-full overflow-hidden rounded-xl bg-surface-2 ring-1 ring-border/60 {selected
      ? 'ring-2 ring-accent'
      : ''}"
  >
    <!-- Art requested as the tile nears the viewport (600px rootMargin) rather
         than for a fixed first-N, so every tile gets art under any sort order.
         requestArt de-dupes internally, so re-asking costs nothing. Every
         entry is requested, not just `game`, so a merged tile can still show a
         Heroic cover when the Steam listing has none cached. -->
    <div
      class="absolute inset-0 {missing
        ? 'opacity-50 grayscale transition-opacity duration-300 group-hover/tile:opacity-90'
        : ''}"
      use:inView={() => (seen = true)}
    >
      {#if art && !artFailed}
        <!-- Fades in when decoded rather than popping in line by line. Keyed on
             src so a swapped cover fades again instead of inheriting `loaded`. -->
        {#key art}
          <img
            src={art}
            alt=""
            decoding="async"
            onload={() => (artLoaded = true)}
            onerror={() => (artFailed = true)}
            class="h-full w-full object-cover transition-[scale,opacity] duration-500 will-change-transform group-hover/tile:scale-[1.04] {artLoaded
              ? 'opacity-100'
              : 'opacity-0'}"
          />
        {/key}
      {:else}
        <span
          class="grid h-full w-full place-items-center text-muted"
          style="background: radial-gradient(120% 80% at 50% 0%, color-mix(in srgb, var(--accent) 12%, transparent), transparent 70%)"
        >
          <GameController size={34} weight="fill" class="opacity-70 transition-[scale,color] duration-300 group-hover/tile:scale-110 group-hover/tile:text-accent" />
        </span>
      {/if}
    </div>

    <!-- Legibility gradient + title. Always shown: it covers placeholder tiles and
         gives Steam art a consistent caption. -->
    <span
      class="pointer-events-none absolute inset-x-0 bottom-0 z-10 bg-gradient-to-t from-black/85 via-black/45 to-transparent px-2.5 pb-2 pt-8"
    >
      <span class="line-clamp-2 text-xs font-medium leading-snug text-white">{game.name}</span>
      {#if playLine}
        <span class="mt-0.5 block truncate text-[10px] leading-tight text-white/65">{playLine}</span>
      {/if}
    </span>

    <!-- The open target: full-bleed, and the tile's accessible name. The sr-only
         span folds the status into that name, so a screen reader announces
         "HELLDIVERS 2, Applied in Steam" rather than leaving the badge invisible
         the way the old title-attribute-only dot did. -->
    <!-- The title carries the status too, so hovering anywhere on the tile explains
         the badge — a better mouse target than the 20px glyph, which stays
         pointer-events-none so it can't swallow a click meant to open the game. -->
    {#if app.selectMode}
      <!-- Batch selection (Library's "Select"): the tile toggles membership
           instead of opening. Picks the primary entry, the one whose tuning
           the tile's badges already describe. -->
      <button
        onclick={() =>
          app.selectedForBatch.has(game.app_id)
            ? app.selectedForBatch.delete(game.app_id)
            : app.selectedForBatch.add(game.app_id)}
        onfocus={() => onactivate(index)}
        tabindex={active ? 0 : -1}
        data-tile={index}
        role="checkbox"
        aria-checked={app.selectedForBatch.has(game.app_id)}
        aria-label="Select {game.name}"
        class="absolute inset-0 z-40 cursor-pointer text-left focus-visible:outline-none {app.selectedForBatch.has(
          game.app_id,
        )
          ? 'ring-2 ring-inset ring-accent'
          : ''}"
      >
        <span
          class="absolute right-2 top-2 grid size-5 place-items-center rounded-md border-2 text-[11px] font-bold {app.selectedForBatch.has(
            game.app_id,
          )
            ? 'border-accent bg-accent text-on-accent'
            : 'border-white/70 bg-black/40 text-transparent'}">✓</span
        >
      </button>
    {:else if multi}
      <!-- More than one way to launch this title (e.g. Steam + a Heroic
           sideload): the open target reveals an in-card picker instead of
           navigating straight away, so the user chooses which one. -->
      <button
        bind:this={triggerEl}
        onclick={() => (pickerOpen = true)}
        onfocus={() => onactivate(index)}
        tabindex={active ? 0 : -1}
        data-tile={index}
        title={titleText}
        aria-haspopup="true"
        aria-expanded={pickerOpen}
        class="absolute inset-0 z-20 cursor-pointer text-left focus-visible:outline-none"
      >
        <span class="sr-only">{srText}</span>
      </button>
      {#if pickerOpen}
        <div
          use:clickOutside={closePicker}
          onkeydown={(e) => {
            if (e.key !== "Escape") return;
            e.stopPropagation();
            closePicker();
          }}
          role="menu"
          tabindex="-1"
          aria-label="Open {game.name} via"
          class="absolute inset-0 z-40 flex flex-col justify-center gap-1.5 bg-black/85 p-2.5"
        >
          <p class="px-1 pb-0.5 text-center text-[10px] font-medium uppercase tracking-wider text-white/70">
            Open via
          </p>
          {#each entries as e, i (e.source + e.app_id)}
            <button
              role="menuitem"
              use:autofocus={i === 0}
              onclick={() => {
                pickerOpen = false;
                app.openGame(e);
              }}
              class="flex items-center gap-2 rounded-lg bg-white/10 px-2.5 py-1.5 text-left text-xs font-medium text-white transition hover:bg-white/20"
            >
              <span
                class="size-2 shrink-0 rounded-full"
                style="background: {sourceColor(e.source)}"
              ></span>
              {sourceLabel(e.source)}
              {#if !e.installed}<span class="ml-auto text-[10px] font-normal text-white/60">not installed</span>{/if}
            </button>
          {/each}
          <button onclick={closePicker} class="mt-1 text-center text-[10px] text-white/60 transition hover:text-white">
            Cancel
          </button>
        </div>
      {/if}
    {:else}
      <button
        onclick={() => app.openGame(game)}
        onfocus={() => onactivate(index)}
        tabindex={active ? 0 : -1}
        data-tile={index}
        title={titleText}
        class="absolute inset-0 z-20 cursor-pointer text-left focus-visible:outline-none"
      >
        <span class="sr-only">{srText}</span>
      </button>
    {/if}

    <!-- Badges sit above the open target so their own titles/labels win on hover,
         but only the star is interactive. -->
    <span class="pointer-events-none absolute left-2 top-2 z-30 flex items-center gap-1">
      {#if missing}
        <span class="rounded-full bg-black/65 px-1.5 py-0.5 text-[10px] font-medium text-white/80"
          >Not installed</span
        >
      {/if}
      {#each extraSources as src (src)}
        <span
          class="rounded-full px-1.5 py-0.5 text-[10px] font-medium"
          style="background: color-mix(in srgb, {sourceColor(src)} 75%, transparent); color: var(--on-accent)"
          >{sourceBadge(src)}</span
        >
      {/each}
      {#if tier}
        <!-- Cache-only: the grid never triggers a lookup, so a screenful of tiles
             can't turn into hundreds of protondb.com requests. Tiles fill in as
             the user visits games. -->
        <span
          class="size-2.5 rounded-full ring-1 ring-black/40"
          style="background: {tierColor(tier.tier)}"
          title="ProtonDB: {tier.tier}"
        ></span>
      {/if}
    </span>

    <span class="absolute right-2 top-2 z-30 flex items-center gap-1">
      {#if status}
        {@const Icon = status.icon}
        <span
          class="pointer-events-none grid size-5 place-items-center rounded-full bg-black/60"
          style="color: {status.colour}"
        >
          <Icon size={13} weight={status.weight} />
        </span>
      {/if}
      <!-- Shares the active tile's tab stop rather than adding one per tile: Tab
           reaches the focused tile, Tab again reaches its star. -->
      <button
        onclick={() => app.toggleFavorite(game.app_id)}
        onfocus={() => onactivate(index)}
        title={favorite ? "Remove from favourites" : "Add to favourites"}
        aria-label={favorite
          ? `Remove ${game.name} from favourites`
          : `Add ${game.name} to favourites`}
        aria-pressed={favorite}
        tabindex={active ? 0 : -1}
        class="grid size-5 place-items-center rounded-full bg-black/60 transition-[opacity,scale] hover:scale-110 {favorite
          ? 'text-yellow opacity-100'
          : 'text-white/70 opacity-0 focus-visible:opacity-100 group-hover/tile:opacity-100'}"
      >
        <Star size={12} weight={favorite ? "fill" : "bold"} />
      </button>
    </span>
  </div>
  <span
    aria-hidden="true"
    class="pointer-events-none absolute inset-0 rounded-xl opacity-0 ring-2 ring-accent glow-accent transition-opacity duration-200 group-hover/tile:opacity-100 group-has-[:focus-visible]/tile:opacity-100"
  ></span>
</div>
