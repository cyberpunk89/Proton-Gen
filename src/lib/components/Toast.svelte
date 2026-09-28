<script lang="ts">
  import { toast, type ToastVariant } from "$lib/toast.svelte";
  import { CheckCircle, WarningCircle, Info, X } from "phosphor-svelte";
  import { fly } from "$lib/motion.svelte";

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
      class="pointer-events-auto relative inline-flex items-center gap-2 overflow-hidden rounded-xl border py-2.5 pl-5 pr-4 shadow-2xl"
      style="border-color: color-mix(in srgb, {COLORS[item.variant]} 40%, transparent);
             background: linear-gradient(90deg, color-mix(in srgb, {COLORS[item.variant]} 10%, var(--surface-solid)), var(--surface-solid) 40%)"
      transition:fly={{ y: 16, duration: 200 }}
    >
      <span class="absolute inset-y-0 left-0 w-1" style="background: {COLORS[item.variant]}" aria-hidden="true"></span>
      <Icon size={16} weight="fill" class="shrink-0 {TEXT[item.variant]}" />
      <span class="text-sm text-text">{item.message}</span>
      {#if item.action}
        <button
          onclick={() => toast.runAction(item.id)}
          class="ml-1 rounded-md px-2 py-0.5 text-xs font-medium text-accent transition hover:bg-surface-2"
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
