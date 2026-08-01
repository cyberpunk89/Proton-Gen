<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { history } from "$lib/history.svelte";
  import { copyCommandAction, resetCommandAction } from "$lib/commands";
  import {
    ArrowCounterClockwise,
    ArrowUUpLeft,
    ArrowUUpRight,
    CheckCircle,
    Copy,
    Terminal,
    WarningCircle,
  } from "phosphor-svelte";
  import { fade } from "$lib/motion.svelte";
  import CommandBody from "./CommandBody.svelte";
  import OpenInSteam from "./OpenInSteam.svelte";
  import SyncPill from "./SyncPill.svelte";

  /**
   * Both actions come from commands.ts so the palette and these buttons cannot
   * drift — the reset-with-undo-toast in particular used to live inline here,
   * which is exactly how the two would have ended up with different toast text
   * or only one of them offering undo.
   *
   * Copy still reads the store's string, never the DOM: even if a selection
   * quirk ever survives in the tokenized body, the primary copy path stays exact
   * by construction.
   */
  const copy = () => void copyCommandAction.run();
  const reset = () => void resetCommandAction.run();

  /**
   * The runtime hint, stated as fact when we can check it and as an instruction
   * when we can't. A game whose compat tool already matches should not be nagged
   * to "change the dropdown" it has already set.
   */
  let runtimeHint = $derived.by(() => {
    const r = app.selectedRuntime;
    if (app.umu || !r) return null;
    if (!app.runtimeComparable) {
      // Generic build, a shortcut, or a valve/auto runtime whose placeholder
      // internal name can never match a config.vdf mapping. Nothing to compare,
      // so the instruction stands unqualified.
      return { done: false, text: `Then set Steam's Proton dropdown to ${r.display_name}` };
    }
    const m = app.runtimeMismatch;
    if (!m) return { done: true, text: `Steam's Proton is already set to ${r.display_name}` };
    return {
      done: false,
      text: m.steam
        ? `Steam's Proton is set to ${m.steam} — change it to ${r.display_name}`
        : `Then set Steam's Proton dropdown to ${r.display_name}`,
    };
  });
</script>

<!--
  `@container`, not viewport breakpoints: this card sits beside a fixed 240px
  NavRail inside a max-w-4xl column, so viewport width overstates the room by a
  constant that a `md:`/`lg:` guess would have to hard-code. Querying the card's
  own width is both correct and stable if the shell layout changes.

  With every label showing the row wants ~730px of content box, and the card's
  content box tops out at 862px, so the full set only has slack on a wide window.
  The tiers shed the least useful text first: the section heading (the Steam/umu
  toggle above already says which mode this is), then the button words, keeping
  Copy's label since it is the primary action.

  Breakpoints are content-box widths, not card widths — a query container is
  measured on its content box, so the card's p-4 costs 32px against every
  threshold. @3xl here means a ~1080px window; @2xl a ~985px one.
-->
<div
  class="@container relative overflow-hidden rounded-2xl border p-4"
  style="border-color: color-mix(in srgb, var(--accent) 30%, transparent);
         background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 7%, var(--mantle)), var(--mantle));"
>
  <div class="mb-2 flex items-center gap-2">
    <Terminal size={15} class="shrink-0 text-accent" />
    <span
      class="hidden shrink-0 text-[11px] font-medium uppercase tracking-wider text-muted @3xl:inline"
    >
      {app.umu ? "umu-launcher command" : "Steam launch options"}
    </span>

    <!-- "Is what I built actually live in Steam?", answered in place. Hides
         itself for umu, shortcuts and no-game. -->
    <SyncPill />

    {#if app.saved}
      <span
        transition:fade={{ duration: 200 }}
        class="hidden shrink-0 text-[11px] text-muted @xl:inline"
        aria-live="polite"
      >
        Saved
      </span>
    {/if}

    <div class="ml-auto flex shrink-0 items-center gap-1.5">
      <!-- Naming the specific action is what makes a stack legible: a disabled
           button with a generic tooltip tells you nothing about where you are. -->
      <button
        onclick={() => app.undo()}
        disabled={!history.canUndo}
        title={history.undoLabel ? `Undo: ${history.undoLabel}` : "Nothing to undo"}
        aria-label={history.undoLabel ? `Undo: ${history.undoLabel}` : "Nothing to undo"}
        class="grid size-7 place-items-center rounded-lg border border-border bg-surface-2/50 text-subtext transition hover:border-accent/50 active:scale-95 disabled:pointer-events-none disabled:opacity-40"
      >
        <ArrowUUpLeft size={14} />
      </button>
      <button
        onclick={() => app.redo()}
        disabled={!history.canRedo}
        title={history.redoLabel ? `Redo: ${history.redoLabel}` : "Nothing to redo"}
        aria-label={history.redoLabel ? `Redo: ${history.redoLabel}` : "Nothing to redo"}
        class="grid size-7 place-items-center rounded-lg border border-border bg-surface-2/50 text-subtext transition hover:border-accent/50 active:scale-95 disabled:pointer-events-none disabled:opacity-40"
      >
        <ArrowUUpRight size={14} />
      </button>

      <!-- Copy, then land on the dialog you paste into. Hides itself when a
           deep link would be meaningless. -->
      <OpenInSteam collapsible />
      <button
        onclick={reset}
        title="Reset the command to defaults"
        aria-label="Reset the command to defaults"
        class="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2 py-1.5 text-xs text-subtext transition hover:border-accent/50 active:scale-95 @2xl:px-2.5"
      >
        <ArrowCounterClockwise size={14} />
        <span class="hidden @2xl:inline">Reset</span>
      </button>
      <button
        onclick={copy}
        class="inline-flex shrink-0 items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition active:scale-95"
        style="background: var(--accent); color: var(--on-accent)"
      >
        <Copy size={14} weight="bold" /> Copy
      </button>
    </div>
  </div>

  <CommandBody {copy} />

  {#if app.buildError}
    <!-- Inline rather than a toast: this re-fires on every keystroke while
         the build is broken, so a toast would spam. -->
    <div
      class="mt-3 flex items-start gap-2 rounded-lg border px-3 py-2 text-xs"
      style="border-color: color-mix(in srgb, var(--red) 35%, transparent);
             background: color-mix(in srgb, var(--red) 8%, transparent)"
      role="alert"
    >
      <WarningCircle size={14} weight="fill" class="mt-0.5 shrink-0 text-red" />
      <div class="flex-1 text-subtext">
        <p class="font-medium text-red">Couldn't build the command</p>
        <p class="mt-0.5 font-mono break-all">{app.buildError}</p>
        <p class="mt-1">The command above may be out of date.</p>
      </div>
      <button
        onclick={() => app.retryBuild()}
        class="shrink-0 rounded-md bg-surface-2 px-2 py-1 text-xs text-subtext transition hover:text-text"
      >
        Retry
      </button>
    </div>
  {/if}

  {#if runtimeHint}
    <p
      class="mt-3 flex items-center gap-1.5 border-t border-border/50 pt-2 text-xs"
      class:text-muted={!runtimeHint.done}
      class:text-green={runtimeHint.done}
    >
      {#if runtimeHint.done}
        <CheckCircle size={13} weight="fill" class="shrink-0" />
      {/if}
      <span>{runtimeHint.text}</span>
    </p>
  {/if}
</div>
