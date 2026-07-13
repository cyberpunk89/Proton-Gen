<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { copyText } from "$lib/util";
  import { toast } from "$lib/toast.svelte";
  import { ArrowCounterClockwise, Copy, Terminal } from "phosphor-svelte";
  import { fade } from "svelte/transition";

  // Svelte JS transitions aren't covered by app.css's global reduced-motion
  // rule, so gate the "Saved" fade explicitly.
  const reduceMotion =
    typeof matchMedia !== "undefined" &&
    matchMedia("(prefers-reduced-motion: reduce)").matches;

  async function copy() {
    await copyText(app.command);
    toast.show("Command copied");
  }

  function reset() {
    const prev = app.resetCommand();
    toast.show("Command reset", {
      action: { label: "Undo", onClick: () => app.loadConfig(prev) },
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
        transition:fade={{ duration: reduceMotion ? 0 : 200 }}
        class="text-[11px] text-muted"
        aria-live="polite"
      >
        Saved
      </span>
    {/if}
    <div class="ml-auto flex items-center gap-1.5">
      <button
        onclick={reset}
        class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent active:scale-95"
      >
        <ArrowCounterClockwise size={14} /> Reset
      </button>
      <button
        onclick={copy}
        class="inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent active:scale-95"
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

  {#if !app.umu && app.selectedRuntime}
    <p class="mt-3 border-t border-border/50 pt-2 text-xs text-muted">
      Then set Steam's Proton dropdown to
      <span class="font-medium text-subtext">{app.selectedRuntime.display_name}</span>
    </p>
  {/if}
</div>
