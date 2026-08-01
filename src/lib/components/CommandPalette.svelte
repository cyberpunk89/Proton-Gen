<script lang="ts">
  import { Dialog as DialogPrimitive } from "bits-ui";
  import { app } from "$lib/state.svelte";
  import { keys } from "$lib/keys.svelte";
  import { buildItems, indexItems, rank, type PaletteItem } from "$lib/palette.svelte";
  import { MagnifyingGlass } from "phosphor-svelte";

  /**
   * Ctrl+K palette over games, parameters, recipes, presets and actions.
   *
   * ## Why this is a hand-rolled combobox
   *
   * This started on bits-ui's `Command`, the obvious choice. Two problems, in
   * order of discovery:
   *
   * 1. `Command` renders every item and filters in the DOM, so a large library
   *    would put thousands of nodes on screen per open. Fixable with
   *    `shouldFilter={false}` plus our own ranking and caps.
   * 2. `Command.Input` owns its handlers and forwards neither `oninput` nor
   *    `onStateChange` to the caller — measured: both fired zero times while the
   *    input's DOM value updated correctly. There is no supported way to read
   *    the search text back out, which makes it unusable when the filtering is
   *    ours.
   *
   * With ranking, caps and search state all ours anyway, `Command` was
   * contributing only listbox ARIA — spelled out explicitly below, where it can
   * be tested. `Dialog` is still bits-ui: its focus trap and layered Escape are
   * worth keeping and work correctly.
   */

  let query = $state("");
  /** Index into `flat`, moved by the arrow keys. */
  let active = $state(0);
  let listEl = $state<HTMLDivElement | null>(null);

  /**
   * Built once per open, not per keystroke — see palette.svelte.ts. Reading
   * `app.showPalette` is what rebuilds it, so reopening picks up anything that
   * changed while it was closed.
   */
  let indexed = $derived(app.showPalette ? indexItems(buildItems()) : []);
  let sections = $derived(rank(indexed, query));
  let flat = $derived(sections.flatMap((s) => s.items));

  $effect(() => {
    if (!app.showPalette) return;
    query = "";
    active = 0;
    keys.pushOverlay();
    return () => keys.popOverlay();
  });

  // A shrinking result set must not leave the highlight past the end.
  $effect(() => {
    if (active >= flat.length) active = 0;
  });

  $effect(() => {
    if (!app.showPalette || !listEl) return;
    void active;
    listEl
      .querySelector<HTMLElement>('[data-active="true"]')
      ?.scrollIntoView({ block: "nearest" });
  });

  function choose(item: PaletteItem, alt: boolean) {
    if (alt && item.runAlt) {
      // Toggling stays open so several options can be flipped in one visit.
      item.runAlt();
      return;
    }
    app.showPalette = false;
    void item.run();
  }

  function onKeydown(e: KeyboardEvent) {
    if (!flat.length) return;
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        active = (active + 1) % flat.length;
        break;
      case "ArrowUp":
        e.preventDefault();
        active = (active - 1 + flat.length) % flat.length;
        break;
      case "Home":
        e.preventDefault();
        active = 0;
        break;
      case "End":
        e.preventDefault();
        active = flat.length - 1;
        break;
      case "Enter": {
        e.preventDefault();
        const item = flat[active];
        // Ctrl+Enter toggles a parameter in place instead of navigating to it.
        if (item) choose(item, e.ctrlKey || e.metaKey);
        break;
      }
    }
  }

  const optionId = (i: number) => `palette-option-${i}`;
</script>

<DialogPrimitive.Root bind:open={app.showPalette}>
  <DialogPrimitive.Portal>
    <DialogPrimitive.Overlay class="fixed inset-0 z-[110] bg-black/50 backdrop-blur-sm" />
    <DialogPrimitive.Content>
      {#snippet child({ props })}
        <div
          class="fixed inset-0 z-[110] flex items-start justify-center p-6 pt-[12vh]"
          role="presentation"
        >
          <div
            {...props}
            class="card w-full max-w-xl overflow-hidden p-0 shadow-2xl"
            style="background: var(--surface-solid)"
          >
            <DialogPrimitive.Title class="sr-only">Command palette</DialogPrimitive.Title>

            <div class="flex items-center gap-2 border-b border-border px-3">
              <MagnifyingGlass size={16} class="shrink-0 text-muted" />
              <!-- svelte-ignore a11y_autofocus -->
              <input
                bind:value={query}
                onkeydown={onKeydown}
                autofocus
                type="text"
                role="combobox"
                aria-expanded="true"
                aria-controls="palette-list"
                aria-activedescendant={flat.length ? optionId(active) : undefined}
                aria-label="Search games, parameters, recipes and actions"
                placeholder="Search games, parameters, recipes, actions…"
                class="w-full bg-transparent py-3 text-sm text-text outline-none placeholder:text-muted"
              />
            </div>

            <div
              bind:this={listEl}
              id="palette-list"
              role="listbox"
              aria-label="Results"
              class="max-h-[50vh] overflow-y-auto p-1.5"
            >
              {#if !flat.length}
                <p class="px-3 py-8 text-center text-sm text-muted">
                  {query.trim() ? `Nothing matches “${query.trim()}”.` : "Nothing to show."}
                </p>
              {/if}

              {#each sections as s (s.group)}
                <div role="group" aria-label={s.label}>
                  <p
                    class="px-2 pb-1 pt-2 text-[10px] font-medium uppercase tracking-wider text-muted"
                  >
                    {s.label}
                    {#if s.total > s.items.length}
                      · {s.items.length} of {s.total}
                    {/if}
                  </p>
                  {#each s.items as item (item.id)}
                    {@const i = flat.indexOf(item)}
                    {@const Icon = item.icon}
                    <!--
                      tabindex="-1", not 0: focus stays in the input and the
                      active option is named by aria-activedescendant, which is
                      the whole point of the pattern. Keyboard handling lives on
                      the input, so the option needs no key handler of its own.
                    -->
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <div
                      id={optionId(i)}
                      role="option"
                      tabindex="-1"
                      aria-selected={i === active}
                      data-active={i === active}
                      onclick={() => choose(item, false)}
                      onmousemove={() => (active = i)}
                      class="flex cursor-pointer items-center gap-2.5 rounded-lg px-2 py-1.5 text-sm {i ===
                      active
                        ? 'bg-surface-2 text-text'
                        : 'text-subtext'}"
                    >
                      {#if Icon}
                        <Icon size={15} class="shrink-0 text-muted" />
                      {/if}
                      <span class="min-w-0 flex-1 truncate">
                        {item.label}
                        {#if item.sublabel}
                          <span class="text-muted"> · {item.sublabel}</span>
                        {/if}
                      </span>
                      {#if item.badge}
                        <span class="shrink-0 text-[10px] text-accent">{item.badge}</span>
                      {/if}
                      {#if item.dimReason}
                        <!-- Dimmed and labelled rather than dropped: a missing
                             entry in a palette reads as a bug. -->
                        <span class="shrink-0 text-[10px] text-peach">{item.dimReason}</span>
                      {/if}
                      {#if item.altLabel && i === active}
                        <span class="shrink-0 font-mono text-[10px] text-muted"
                          >⌃⏎ {item.altLabel}</span
                        >
                      {/if}
                    </div>
                  {/each}
                </div>
              {/each}
            </div>

            <div
              class="flex items-center gap-3 border-t border-border px-3 py-1.5 text-[10px] text-muted"
            >
              <span><kbd class="font-mono">↑↓</kbd> navigate</span>
              <span><kbd class="font-mono">⏎</kbd> open</span>
              <span><kbd class="font-mono">Ctrl+⏎</kbd> toggle</span>
              <span class="ml-auto"><kbd class="font-mono">Esc</kbd> close</span>
            </div>
          </div>
        </div>
      {/snippet}
    </DialogPrimitive.Content>
  </DialogPrimitive.Portal>
</DialogPrimitive.Root>
