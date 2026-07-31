<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { copyText } from "$lib/util";
  import { toast } from "$lib/toast.svelte";
  import { ArrowCounterClockwise, Copy, Terminal, WarningCircle } from "phosphor-svelte";
  import { fade } from "$lib/motion.svelte";
  import OpenInSteam from "./OpenInSteam.svelte";

  async function copy() {
    await copyText(app.command);
    toast.success("Command copied");
  }

  function reset() {
    app.resetCommand();
    // The action now routes through the real stack, so it is no longer the only
    // way back — Ctrl+Z still works after this toast expires. Undo/redo buttons
    // land with the command-bar rewrite (#37).
    toast.success("Command reset", {
      action: { label: "Undo", onClick: () => app.undo() },
    });
  }
</script>

<div
  class="relative overflow-hidden rounded-2xl border p-4"
  style="border-color: color-mix(in srgb, var(--accent) 30%, transparent);
         background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 7%, var(--mantle)), var(--mantle));"
>
  <div class="mb-2 flex items-center gap-2">
    <Terminal size={15} class="text-accent" />
    <span class="text-[11px] font-medium uppercase tracking-wider text-muted">
      {app.umu ? "umu-launcher command" : "Steam launch options"}
    </span>
    {#if app.saved}
      <span
        transition:fade={{ duration: 200 }}
        class="text-[11px] text-muted"
        aria-live="polite"
      >
        Saved
      </span>
    {/if}
    <div class="ml-auto flex items-center gap-1.5">
      <!-- Copy, then land on the dialog you paste into. Hides itself when a
           deep link would be meaningless. -->
      <OpenInSteam />
      <button
        onclick={reset}
        class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50 active:scale-95"
      >
        <ArrowCounterClockwise size={14} /> Reset
      </button>
      <button
        onclick={copy}
        class="inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition active:scale-95"
        style="background: var(--accent); color: var(--on-accent)"
      >
        <Copy size={14} weight="bold" /> Copy
      </button>
    </div>
  </div>

  <button
    onclick={copy}
    class="block w-full cursor-copy select-text text-left font-mono text-[13px] leading-relaxed text-text"
    style="word-break: break-word"
  >
    {app.command || "%command%"}
  </button>

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

  {#if !app.umu && app.selectedRuntime}
    <p class="mt-3 border-t border-border/50 pt-2 text-xs text-muted">
      Then set Steam's Proton dropdown to
      <span class="font-medium text-subtext">{app.selectedRuntime.display_name}</span>
    </p>
  {/if}
</div>
