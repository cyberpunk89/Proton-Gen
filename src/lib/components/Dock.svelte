<script lang="ts">
  import { app } from "$lib/state.svelte";
  import CommandPreview from "./CommandPreview.svelte";
  import ProblemsPane from "./ProblemsPane.svelte";
  import RunsPane from "./RunsPane.svelte";
  import Lazy from "./Lazy.svelte";
  import { lazy } from "$lib/lazy";
  import { Terminal, WarningOctagon, FileText, CaretDown, CaretUp, Pulse } from "phosphor-svelte";

  /**
   * The bottom dock: one place for everything about the output. Collapsed
   * (the Command tab) it's the command bar it replaced; Problems and Logs pull
   * it up to a resizable pane. A pane, not a `Dialog`, so it needs no
   * `restoreScrollDelay` and can't strand a pointer-events lock (#63).
   */
  type Tab = "command" | "problems" | "logs" | "runs";

  // Runs for the open game: on entering it, and every few seconds while the
  // Runs tab is up (a game writes its log as it plays). Focus polls too.
  $effect(() => {
    void app.selectedAppId;
    void app.pollRuns();
  });
  $effect(() => {
    if (app.dockTab !== "runs") return;
    const t = setInterval(() => void app.pollRuns(), 5000);
    return () => clearInterval(t);
  });

  let counts = $derived({
    error: app.notices.filter((n) => n.severity === "error").length,
    rest: app.notices.filter((n) => n.severity !== "error").length,
  });

  let dock = $state<HTMLElement | null>(null);
  let dragging = $state(false);

  function select(t: Tab) {
    app.dockTab = app.dockTab === t && t !== "command" ? "command" : t;
  }

  /** Drag the top edge. Clamped so the builder keeps some room above. */
  function startResize(e: PointerEvent) {
    if (!dock) return;
    e.preventDefault();
    const bottom = dock.getBoundingClientRect().bottom;
    dragging = true;
    const move = (ev: PointerEvent) => {
      const h = bottom - ev.clientY;
      app.dockHeight = Math.round(Math.min(window.innerHeight * 0.75, Math.max(180, h)));
    };
    const up = () => {
      dragging = false;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function onKeyResize(e: KeyboardEvent) {
    const step = e.shiftKey ? 80 : 24;
    if (e.key === "ArrowUp") app.dockHeight = Math.min(window.innerHeight * 0.75, app.dockHeight + step);
    else if (e.key === "ArrowDown") app.dockHeight = Math.max(180, app.dockHeight - step);
    else return;
    e.preventDefault();
  }

  let open = $derived(app.dockTab !== "command");
</script>

<div
  bind:this={dock}
  class="relative flex shrink-0 flex-col border-t border-border bg-mantle/30 {dragging ? 'select-none' : ''}"
  style={open ? `height: ${app.dockHeight}px` : ""}
>
  {#if open}
    <!-- A focusable separator is a resize widget in ARIA; Svelte's lint
         doesn't know that role can be interactive. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      role="separator"
      aria-orientation="horizontal"
      aria-label="Resize the dock"
      aria-valuenow={app.dockHeight}
      aria-valuemin={180}
      tabindex="0"
      onpointerdown={startResize}
      onkeydown={onKeyResize}
      class="absolute inset-x-0 -top-1 z-10 h-2 cursor-row-resize hover:bg-accent/30 focus-visible:bg-accent/30"
    ></div>
  {/if}

  <div class="flex items-center gap-1 border-b border-border/60 px-4" role="tablist" aria-label="Dock">
    {@render tab("command", "Command", Terminal)}
    {@render tab("problems", "Problems", WarningOctagon)}
    {@render tab("logs", "Logs", FileText)}
    {@render tab("runs", "Runs", Pulse)}
    <button
      onclick={() => (app.dockTab = open ? "command" : "problems")}
      class="ml-auto grid size-7 place-items-center rounded-md text-muted transition hover:bg-surface-2 hover:text-text"
      aria-label={open ? "Collapse the dock" : "Expand the dock"}
      title={open ? "Collapse" : "Expand"}
    >
      {#if open}<CaretDown size={14} />{:else}<CaretUp size={14} />{/if}
    </button>
  </div>

  {#if !open}
    <div class="px-5 py-3">
      <div class="mx-auto max-w-5xl"><CommandPreview /></div>
    </div>
  {:else}
    <div class="min-h-0 flex-1 overflow-y-auto px-5 py-3" role="tabpanel">
      <div class="mx-auto max-w-6xl">
        {#if app.dockTab === "problems"}
          <ProblemsPane />
        {:else if app.dockTab === "runs"}
          <RunsPane />
        {:else}
          <Lazy load={lazy.logViewer}>
            {#snippet children(C)}<C />{/snippet}
          </Lazy>
        {/if}
      </div>
    </div>
  {/if}
</div>

{#snippet tab(id: Tab, label: string, Icon: typeof Terminal)}
  {@const active = app.dockTab === id}
  <button
    role="tab"
    aria-selected={active}
    onclick={() => select(id)}
    class="relative inline-flex items-center gap-1.5 px-3 py-2 text-xs transition {active
      ? 'font-medium text-text'
      : 'text-muted hover:text-subtext'}"
  >
    <Icon size={13} weight={active ? "fill" : "regular"} />
    {label}
    {#if id === "problems"}
      {#if counts.error}
        <span class="rounded-full bg-red/20 px-1.5 text-[10px] text-red">{counts.error}</span>
      {/if}
      {#if counts.rest}
        <span class="rounded-full bg-peach/20 px-1.5 text-[10px] text-peach">{counts.rest}</span>
      {/if}
    {/if}
    {#if id === "runs" && app.runs.length}
      {@const latest = app.runs[0]}
      <span
        class="size-1.5 rounded-full"
        title="Latest run"
        style="background: {latest.mark === true || (latest.mark == null && latest.verdict === 'clean')
          ? 'var(--green)'
          : latest.verdict === 'errors' && latest.mark == null
            ? 'var(--peach)'
            : 'var(--red)'}"
      ></span>
    {/if}
    {#if active}
      <span class="absolute inset-x-2 -bottom-px h-0.5 rounded-full bg-accent"></span>
    {/if}
  </button>
{/snippet}
