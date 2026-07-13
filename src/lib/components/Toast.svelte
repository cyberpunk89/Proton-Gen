<script lang="ts">
  import { toast } from "$lib/toast.svelte";
  import { CheckCircle } from "phosphor-svelte";
  import { fly } from "svelte/transition";
</script>

{#if toast.message}
  <div
    class="fixed bottom-6 right-6 z-[200] inline-flex items-center gap-2 rounded-xl border px-4 py-2.5 shadow-2xl"
    style="border-color: color-mix(in srgb, var(--green) 40%, transparent);
           background: var(--surface-solid)"
    transition:fly={{ y: 16, duration: 200 }}
  >
    <CheckCircle size={16} weight="fill" class="text-green" />
    <span class="text-sm text-text">{toast.message}</span>
    {#if toast.action}
      <button
        onclick={() => toast.runAction()}
        class="ml-1 rounded-md px-2 py-0.5 text-xs font-medium text-accent transition hover:bg-surface-2 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
      >
        {toast.action.label}
      </button>
    {/if}
  </div>
{/if}
