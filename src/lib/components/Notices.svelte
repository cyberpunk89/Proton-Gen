<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { Warning } from "phosphor-svelte";
  import { slide } from "svelte/transition";

  let open = $state(true);
</script>

{#if app.notices.length}
  <div
    class="rounded-xl border p-3"
    style="border-color: color-mix(in srgb, var(--peach) 35%, transparent);
           background: color-mix(in srgb, var(--peach) 8%, transparent)"
    transition:slide={{ duration: 150 }}
  >
    <button class="flex w-full items-center gap-2" onclick={() => (open = !open)}>
      <Warning size={15} weight="fill" class="text-peach" />
      <span class="text-xs font-medium text-peach"
        >{app.notices.length} notice{app.notices.length > 1 ? "s" : ""}</span
      >
      <span class="ml-auto text-[11px] text-muted">{open ? "hide" : "show"}</span>
    </button>
    {#if open}
      <ul class="mt-2 space-y-1 pl-6 text-xs text-subtext" transition:slide={{ duration: 120 }}>
        {#each app.notices as n (n)}
          <li class="list-disc">{n}</li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}
