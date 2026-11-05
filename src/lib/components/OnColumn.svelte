<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import { splitExtraEnv } from "$lib/shell";
  import { irrelevance } from "$lib/util";
  import { X, Warning, FloppyDisk, CaretRight, SpeakerSlash, SpeakerHigh } from "phosphor-svelte";
  import type { Severity } from "$lib/types";

  /**
   * "On for this game": everything the command will carry, in emit order —
   * runtime, wrappers, environment, custom env, game args — always on screen
   * beside the catalog (Everything) or the cards (Essentials). Replaces the
   * Active options page, which you had to navigate to.
   *
   * Each item says where it came from (a recipe, or you), whether lint has
   * something to say about it, and whether your hardware can use it. Clicking
   * one jumps to its catalog row; the speaker mutes it (stays here, leaves the
   * command — for bisecting); × turns it off. A/B keeps two variants of the
   * whole stack per game to flip between.
   */
  let mutedCount = $derived(app.muted.filter((k) => app.env[k]?.enabled || app.wrap[k]?.enabled).length);

  let wrappers = $derived(app.catalog.wrappers.filter((w) => app.wrap[w.key]?.enabled));
  let envs = $derived(app.catalog.envs.filter((e) => app.env[e.key]?.enabled));
  let custom = $derived(splitExtraEnv(app.extraEnv));
  let gameArgs = $derived(app.gameArgs.trim());

  const SEV: Record<Severity, string> = {
    error: "var(--red)",
    warning: "var(--peach)",
    info: "var(--blue)",
  };

  let flash = $state(false);
  $effect(() => {
    if (!app.onColumnFlash) return;
    flash = true;
    const t = setTimeout(() => (flash = false), 900);
    return () => clearTimeout(t);
  });

  function off(kind: "env" | "wrap" | "custom", key: string) {
    if (kind === "env") app.toggleEnv(key);
    else if (kind === "wrap") app.toggleWrap(key);
    else app.removeExtraEnv(key);
    const name = kind === "custom" ? key.split("=")[0] : key;
    toast.success(`Turned off ${name}`, { action: { label: "Undo", onClick: () => app.undo() } });
  }

  function clearAll() {
    app.resetCommand();
    toast.success("Cleared all options", { action: { label: "Undo", onClick: () => app.undo() } });
  }

  /** Jump to the row; on a narrow window the overlay gets out of the way. */
  function go(key: string) {
    if (app.revealParam(key)) app.onColumnOpen = false;
  }
</script>

<aside
  aria-label="On for this game"
  class="flex h-full w-64 flex-col border-r border-border transition-shadow {flash
    ? 'ring-2 ring-inset ring-accent'
    : ''}"
  style="background: var(--mantle)"
>
  <div class="flex items-center gap-2 px-4 pb-2 pt-3">
    <h2 class="text-sm font-medium text-text">On for this game</h2>
    <span
      class="rounded-full px-1.5 text-[10px]"
      style="background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent)"
      >{app.activeCount}</span
    >
    {#if app.selectedAppId != null}
      <!-- A/B: two variants of this game's stack. B starts as a copy of A. -->
      <div
        class="ml-auto inline-flex items-center rounded-md border border-border p-0.5"
        role="group"
        aria-label="A/B slot"
        title={app.abSlot ? "Flip between this game's two variants" : "Make a B variant to compare against"}
      >
        {#each ["A", "B"] as const as side (side)}
          <button
            onclick={() => app.switchAB(side)}
            aria-pressed={app.abSide === side}
            class="rounded px-1.5 text-[11px] font-medium transition {app.abSide === side
              ? 'bg-accent text-on-accent'
              : 'text-muted hover:text-text'}">{side}</button
          >
        {/each}
      </div>
      {#if app.abSlot}
        <button
          onclick={() => app.dropAB()}
          class="grid size-5 place-items-center rounded text-muted hover:text-text"
          aria-label="Keep only {app.abSide}"
          title="Keep only {app.abSide}, discard the other"
        >
          <X size={11} />
        </button>
      {/if}
    {/if}
    <button
      onclick={() => (app.onColumnOpen = false)}
      class="grid size-6 place-items-center rounded-md text-muted hover:bg-surface-2 hover:text-text min-[1100px]:hidden"
      aria-label="Close"
    >
      <X size={13} />
    </button>
  </div>

  <div class="min-h-0 flex-1 space-y-3 overflow-y-auto px-2 pb-3">
    {@render group("Runtime")}
    <button
      onclick={() => app.setSection("game")}
      class="group flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left font-mono text-[12px] text-subtext transition hover:bg-surface-2/60"
      title="Game & runtime"
    >
      <span class="min-w-0 flex-1 truncate">{app.selectedRuntime?.display_name ?? "no runtime"}</span>
      <span class="shrink-0 font-sans text-[10px] text-muted">{app.umu ? "umu" : "Steam"}</span>
      <CaretRight size={11} class="shrink-0 text-muted" />
    </button>

    {#if wrappers.length}
      <div>
        {@render group("Wrappers")}
        {#each wrappers as w (w.key)}
          {@render item("wrap", w.key, w.label ?? w.key, app.wrap[w.key]?.value ?? "", irrelevance(app.hwCaps, w.gpu, w.needs))}
        {/each}
      </div>
    {/if}

    {#if envs.length}
      <div>
        {@render group("Environment")}
        {#each envs as e (e.key)}
          {@render item("env", e.key, e.key, app.env[e.key]?.value ?? "", irrelevance(app.hwCaps, e.gpu, e.needs))}
        {/each}
      </div>
    {/if}

    {#if custom.length}
      <div>
        {@render group("Custom env")}
        {#each custom as c (c.raw)}
          {@render item("custom", c.raw, c.key, c.value, null)}
        {/each}
      </div>
    {/if}

    {#if gameArgs}
      <div>
        {@render group("Game args")}
        <button
          onclick={() => app.setSection("game")}
          class="w-full truncate rounded-lg px-2 py-1.5 text-left font-mono text-[12px] text-subtext hover:bg-surface-2/60"
          >{gameArgs}</button
        >
      </div>
    {/if}

    {#if app.activeCount === 0}
      <p class="px-2 py-4 text-xs leading-relaxed text-muted">
        Nothing on yet. Turn options on in the catalog, or
        <button onclick={() => app.setSection("recipes")} class="text-accent hover:underline"
          >apply a recipe</button
        >.
      </p>
    {/if}
  </div>

  <div class="space-y-2 border-t border-border/60 p-3">
    <div class="flex items-center gap-3 px-1 text-[10px] text-muted">
      <span class="inline-flex items-center gap-1"><span class="size-1.5 rounded-full bg-mauve"></span> recipe</span>
      <span class="inline-flex items-center gap-1"><span class="size-1.5 rounded-full bg-muted"></span> manual</span>
      <span class="inline-flex items-center gap-1"><Warning size={10} weight="fill" class="text-peach" /> check</span>
    </div>
    <div class="flex gap-2">
      <button
        onclick={() => (app.showSave = true)}
        class="inline-flex flex-1 items-center justify-center gap-1.5 rounded-lg border border-border px-2 py-1.5 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
      >
        <FloppyDisk size={13} /> Save as preset
      </button>
      {#if mutedCount}
        <button
          onclick={() => {
            app.muted = [];
            toast.success("Unmuted everything");
          }}
          class="rounded-lg border border-border px-2 py-1.5 text-xs text-subtext transition hover:border-accent/50"
          aria-label="Unmute all {mutedCount}"
          title="Unmute all {mutedCount}"
        >
          <SpeakerHigh size={13} />
        </button>
      {/if}
      <button
        onclick={clearAll}
        disabled={app.activeCount === 0}
        class="rounded-lg border border-border px-2 py-1.5 text-xs text-subtext transition hover:border-red/50 hover:text-red disabled:opacity-40"
      >
        Clear
      </button>
    </div>
  </div>
</aside>

{#snippet group(label: string)}
  <p class="px-2 pb-0.5 text-[10px] font-medium uppercase tracking-wider text-muted">{label}</p>
{/snippet}

{#snippet item(
  kind: "env" | "wrap" | "custom",
  key: string,
  label: string,
  value: string,
  hwWarning: string | null,
)}
  {@const notices = kind === "custom" ? [] : (app.noticesByKey.get(key) ?? [])}
  {@const recipe = kind === "custom" ? null : (app.recipeOrigin[key] ?? null)}
  {@const muted = kind !== "custom" && app.isMuted(key)}
  <div class="group flex items-center rounded-lg transition hover:bg-surface-2/60 {muted ? 'opacity-55' : ''}">
    <button
      onclick={() => kind !== "custom" && go(key)}
      disabled={kind === "custom"}
      class="flex min-w-0 flex-1 items-center gap-2 px-2 py-1 text-left disabled:cursor-default"
      title={[
        recipe ? `Set by the “${recipe}” recipe` : "Turned on by you",
        ...notices.map((n) => n.message),
        hwWarning,
      ]
        .filter(Boolean)
        .join("\n")}
    >
      <!-- Source dot: recipe or manual. A lint notice recolours it. -->
      <span
        class="size-1.5 shrink-0 rounded-full"
        style="background: {notices.length
          ? SEV[notices[0].severity]
          : recipe
            ? 'var(--mauve)'
            : 'var(--muted)'}"
      ></span>
      <span class="min-w-0 flex-1 truncate font-mono text-[12px] text-text {muted ? 'line-through' : ''}">
        {label}{#if value}<span class="text-muted">{kind === "wrap" ? " " : "="}{value}</span>{/if}
      </span>
      {#if hwWarning}
        <Warning size={11} weight="fill" class="shrink-0 text-peach" />
      {/if}
    </button>
    {#if kind !== "custom"}
      <button
        onclick={() => app.toggleMute(key)}
        aria-label="{muted ? 'Unmute' : 'Mute'} {label}"
        aria-pressed={muted}
        title={muted ? "Unmute — put it back in the command" : "Mute — keep it here, leave it out of the command"}
        class="grid size-5 shrink-0 place-items-center rounded transition hover:text-text focus-visible:opacity-100 group-hover:opacity-100 {muted
          ? 'text-peach opacity-100'
          : 'text-muted opacity-0'}"
      >
        <SpeakerSlash size={11} weight={muted ? "fill" : "regular"} />
      </button>
    {/if}
    <button
      onclick={() => off(kind, key)}
      aria-label="Turn off {label}"
      title="Turn off"
      class="mr-1 grid size-5 shrink-0 place-items-center rounded text-muted opacity-0 transition hover:text-red focus-visible:opacity-100 group-hover:opacity-100"
    >
      <X size={11} weight="bold" />
    </button>
  </div>
{/snippet}
