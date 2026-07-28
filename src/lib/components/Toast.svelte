<script lang="ts">
  import { toast, type ToastVariant } from "$lib/toast.svelte";
  import { CheckCircle, WarningCircle, Info, X } from "phosphor-svelte";
  import { fly } from "svelte/transition";

  const ICONS = {
    success: CheckCircle,
    error: WarningCircle,
    info: Info,
  };

  const COLORS: Record<ToastVariant, string> = {
    success: "var(--green)",
    error: "var(--red)",
    info: "var(--blue)",
  };

  const TEXT: Record<ToastVariant, string> = {
    success: "text-green",
    error: "text-red",
    info: "text-blue",
  };

  // Errors interrupt; the rest wait their turn in the queue.
  let hasError = $derived(toast.items.some((t) => t.variant === "error"));
</script>

<!--
  One live region for the whole stack rather than one per toast: a region
  added to the DOM at the same moment as its content is not reliably
  announced, since the assistive tech has nothing to diff against.
-->
<div
  class="pointer-events-none fixed bottom-6 right-6 z-[200] flex flex-col items-end gap-2"
  role={hasError ? "alert" : "status"}
  aria-live={hasError ? "assertive" : "polite"}
  aria-atomic="false"
>
  {#each toast.items as item (item.id)}
    {@const Icon = ICONS[item.variant]}
    <div
      class="pointer-events-auto inline-flex items-center gap-2 rounded-xl border px-4 py-2.5 shadow-2xl"
      style="border-color: color-mix(in srgb, {COLORS[item.variant]} 40%, transparent);
             background: var(--surface-solid)"
      transition:fly={{ y: 16, duration: 200 }}
    >
      <Icon size={16} weight="fill" class="shrink-0 {TEXT[item.variant]}" />
      <span class="text-sm text-text">{item.message}</span>
      {#if item.action}
        <button
          onclick={() => toast.runAction(item.id)}
          class="ml-1 rounded-md px-2 py-0.5 text-xs font-medium text-accent transition hover:bg-surface-2 focus-visible:outline focus-visible:outline-2 focus-visible:outline-accent"
        >
          {item.action.label}
        </button>
      {/if}
      <button
        onclick={() => toast.dismiss(item.id)}
        class="ml-0.5 shrink-0 text-muted transition hover:text-text"
        aria-label="Dismiss"
      >
        <X size={13} />
      </button>
    </div>
  {/each}
</div>
