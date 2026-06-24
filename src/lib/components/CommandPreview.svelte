<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { copyText } from "$lib/util";
  import { toast } from "$lib/toast.svelte";
  import { Copy, Terminal } from "phosphor-svelte";

  async function copy() {
    await copyText(app.command);
    toast.show("Command copied");
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
    <button
      onclick={copy}
      class="ml-auto inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition active:scale-95"
      style="background: var(--accent); color: var(--on-accent)"
    >
      <Copy size={14} weight="bold" /> Copy
    </button>
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
