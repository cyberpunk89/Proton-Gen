<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { fly } from "svelte/transition";
  import { clickOutside } from "$lib/actions";
  import { Cpu, CaretUpDown, Check } from "phosphor-svelte";
  import type { RuntimeDto } from "$lib/types";

  let open = $state(false);

  function choose(r: RuntimeDto) {
    app.selectedRuntime = r;
    open = false;
  }

  const kindLabel: Record<string, string> = {
    system: "system",
    user: "user",
    valve: "valve",
  };
</script>

<div class="relative" use:clickOutside={() => (open = false)}>
  <button
    type="button"
    onclick={() => (open = !open)}
    class="flex w-full items-center gap-2 rounded-xl border border-border bg-surface-2/60 px-3 py-3 text-left transition hover:border-accent/40"
    title="Proton runtime"
  >
    <Cpu size={16} class="shrink-0 text-muted" />
    <span class="min-w-0 flex-1">
      <span class="block text-[11px] uppercase tracking-wider text-muted">Proton</span>
      <span class="block truncate text-sm text-subtext">
        {app.selectedRuntime?.display_name ?? "—"}
      </span>
    </span>
    <CaretUpDown size={16} class="shrink-0 text-muted" />
  </button>

  {#if open}
    <div
      transition:fly={{ y: -4, duration: 120 }}
      class="popover absolute right-0 top-full z-50 mt-2 max-h-[340px] w-[360px] overflow-y-auto p-1.5"
    >
      {#each app.runtimes as r (r.display_name)}
        <button
          onclick={() => choose(r)}
          class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left hover:bg-surface-2"
        >
          {#if app.selectedRuntime?.display_name === r.display_name}<Check
              size={14}
              class="shrink-0 text-accent"
            />{:else}<span class="size-3.5 shrink-0"></span>{/if}
          <span class="min-w-0 flex-1 truncate text-sm text-text">{r.display_name}</span>
          <span
            class="shrink-0 rounded-full px-2 py-0.5 text-[10px]"
            style="background: color-mix(in srgb, var(--blue) 16%, transparent); color: var(--blue)"
            >{kindLabel[r.kind] ?? r.kind}</span
          >
        </button>
      {:else}
        <p class="px-3 py-6 text-center text-sm text-muted">No runtimes found.</p>
      {/each}
    </div>
  {/if}
</div>
