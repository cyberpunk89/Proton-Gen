<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { focusTarget } from "$lib/actions";
  import CurrentGameCard from "./CurrentGameCard.svelte";
  import ModeToggle from "./ModeToggle.svelte";
  import {
    Sparkle,
    Cpu,
    MagnifyingGlass,
    X,
    Faders,
  } from "phosphor-svelte";

  let wrapCount = $derived(
    app.catalog.wrappers.filter((w) => app.wrap[w.key]?.enabled).length,
  );

  // A nav item is "active" only when it's the current section AND the user
  // isn't mid-search (search overrides the panel).
  function isActive(section: string): boolean {
    return app.activeSection === section && app.paramQuery.trim() === "";
  }
</script>

<aside class="flex w-60 shrink-0 flex-col border-r border-border bg-mantle/30">
  <div class="flex flex-col gap-2.5 p-3">
    <CurrentGameCard />
    <div class="flex"><ModeToggle /></div>
    <div class="relative">
      <MagnifyingGlass
        size={14}
        class="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"
      />
      <input
        use:focusTarget={"param-search"}
        bind:value={app.paramQuery}
        aria-label="Search parameters"
        placeholder="Search parameters…"
        class="w-full rounded-lg border border-border bg-surface-2 py-1.5 pl-8 pr-7 text-xs text-text outline-none focus:border-accent"
      />
      {#if app.paramQuery}
        <button
          onclick={() => (app.paramQuery = "")}
          class="absolute right-2 top-1/2 -translate-y-1/2 text-muted hover:text-text"
          aria-label="Clear search"
        >
          <X size={13} />
        </button>
      {/if}
    </div>
  </div>

  <nav class="min-h-0 flex-1 space-y-0.5 overflow-y-auto px-2 pb-3">
    {@render navItem("recipes", "Recipes", Sparkle)}

    <p class="px-2 pb-1 pt-3 text-[10px] font-medium uppercase tracking-wider text-muted">
      Parameters
    </p>
    {@render catItem("Wrappers", wrapCount)}
    {#each app.categories as c (c)}
      {@render catItem(c, app.enabledCountInCategory(c))}
    {/each}

    <p class="px-2 pb-1 pt-3 text-[10px] font-medium uppercase tracking-wider text-muted">
      Setup
    </p>
    {@render navItem("game", "Game & runtime", Cpu)}
  </nav>
</aside>

{#snippet navItem(section: string, label: string, Icon: typeof Sparkle)}
  {@const active = isActive(section)}
  <button
    onclick={() => app.setSection(section)}
    class="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[13px] transition {active
      ? ''
      : 'text-subtext hover:bg-surface-2'}"
    style={active
      ? "background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent)"
      : ""}
  >
    <Icon size={16} weight={active ? "fill" : "regular"} class="shrink-0" />
    <span class="truncate">{label}</span>
  </button>
{/snippet}

{#snippet catItem(name: string, count: number)}
  {@const active = isActive(name)}
  <button
    onclick={() => app.setSection(name)}
    class="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[13px] transition {active
      ? ''
      : 'text-subtext hover:bg-surface-2'}"
    style={active
      ? "background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent)"
      : ""}
  >
    <Faders size={16} class="shrink-0 {active ? '' : 'text-muted'}" />
    <span class="min-w-0 flex-1 truncate">{name}</span>
    {#if count > 0}
      <span
        class="shrink-0 rounded-full px-1.5 text-[10px]"
        style={active
          ? "background: color-mix(in srgb, var(--accent) 22%, transparent)"
          : "background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent)"}
        >{count}</span
      >
    {/if}
  </button>
{/snippet}
