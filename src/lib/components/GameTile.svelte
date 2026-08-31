<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { inView, clickOutside, autofocus } from "$lib/actions";
  import { keys } from "$lib/keys.svelte";
  import { tierColor } from "$lib/util";
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
  let multi = $derived(entries.length > 1);

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
  let art = $derived.by(() => {
    for (const e of entries) {
      const a = app.artFor(e.app_id, e.source, "portrait");
      if (a) return a;
    }
    return undefined;
  });

  let selected = $derived(entries.some((e) => app.selectedAppId === e.app_id));
  let favorite = $derived(app.isFavorite(game.app_id));

  /** Cached only — see `tier` below. */
  let tier = $derived(app.tierFor(game.app_id));

  /** Every source represented in this tile besides Steam's (which gets no
   *  badge of its own, same as a single-source Steam tile today). */
  let extraSources = $derived([...new Set(entries.map((e) => e.source))].filter((s) => s !== "steam"));

  function sourceLabel(source: string): string {
    switch (source) {
      case "steam":
        return "Steam";
      case "non-steam":
        return "Non-Steam shortcut";
      case "heroic":
        return "Heroic";
      default:
        return source;
    }
  }

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

  /** Hover/accessible text for the open target — folds in the multi-source
   *  hint so "there's a choice here" isn't badge-only information. */
  let openDescription = $derived(
    multi ? `choose ${entries.map((e) => sourceLabel(e.source)).join(" or ")} to open` : "",
  );
  let titleText = $derived(
    [game.name, status?.label, openDescription].filter(Boolean).join(" — "),
  );
  let srText = $derived(
    `${game.name}${status ? `, ${status.label}` : ""}${favorite ? ", favourite" : ""}${
      openDescription ? `, ${openDescription}` : ""
    }`,
  );
</script>

<div
  class="group/tile relative aspect-[2/3] overflow-hidden rounded-xl bg-surface-2 ring-1 ring-border/60 transition duration-150 hover:-translate-y-1 hover:ring-2 hover:ring-accent has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-accent {selected
    ? 'ring-2 ring-accent'
    : ''}"
>
  <!-- Art requested as the tile nears the viewport (600px rootMargin) rather
       than for a fixed first-N, so every tile gets art under any sort order.
       requestArt de-dupes internally, so re-observing costs nothing. Every
       entry is requested, not just `game`, so a merged tile can still show a
       Heroic cover when the Steam listing has none cached. -->
  <div
    class="absolute inset-0"
    use:inView={() => entries.forEach((e) => app.requestArt(e.app_id, e.source, "portrait", e.art_url))}
  >
    {#if art}
      <img
        src={art}
        alt=""
        loading="lazy"
        class="h-full w-full object-cover transition duration-300 group-hover/tile:scale-[1.04]"
      />
    {:else}
      <span class="grid h-full w-full place-items-center text-muted">
        <GameController size={34} weight="fill" />
      </span>
    {/if}
  </div>

  <!-- Legibility gradient + title. Always shown: it covers placeholder tiles and
       gives Steam art a consistent caption. -->
  <span
    class="pointer-events-none absolute inset-x-0 bottom-0 z-10 bg-gradient-to-t from-black/85 via-black/45 to-transparent px-2.5 pb-2 pt-8"
  >
    <span class="line-clamp-2 text-xs font-medium leading-snug text-white">{game.name}</span>
  </span>

  <!-- The open target: full-bleed, and the tile's accessible name. The sr-only
       span folds the status into that name, so a screen reader announces
       "HELLDIVERS 2, Applied in Steam" rather than leaving the badge invisible
       the way the old title-attribute-only dot did. -->
  <!-- The title carries the status too, so hovering anywhere on the tile explains
       the badge — a better mouse target than the 20px glyph, which stays
       pointer-events-none so it can't swallow a click meant to open the game. -->
  {#if multi}
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
        class="absolute inset-0 z-40 flex flex-col justify-center gap-1.5 bg-black/80 p-2.5 backdrop-blur-sm"
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
              style="background: {e.source === 'heroic'
                ? 'var(--blue)'
                : e.source === 'non-steam'
                  ? 'var(--mauve)'
                  : 'var(--accent)'}"
            ></span>
            {sourceLabel(e.source)}
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
    {#each extraSources as src (src)}
      <span
        class="rounded-full px-1.5 py-0.5 text-[10px] font-medium backdrop-blur-sm"
        style="background: color-mix(in srgb, {src === 'heroic'
          ? 'var(--blue)'
          : 'var(--mauve)'} 75%, transparent); color: var(--on-accent)"
        >{src === "heroic" ? "Heroic" : "shortcut"}</span
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
        class="pointer-events-none grid size-5 place-items-center rounded-full bg-black/45 backdrop-blur-sm"
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
      class="grid size-5 place-items-center rounded-full bg-black/45 backdrop-blur-sm transition hover:scale-110 {favorite
        ? 'text-yellow opacity-100'
        : 'text-white/70 opacity-0 focus-visible:opacity-100 group-hover/tile:opacity-100'}"
    >
      <Star size={12} weight={favorite ? "fill" : "bold"} />
    </button>
  </span>
</div>
