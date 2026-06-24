<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    open = $bindable(false),
    title,
    subtitle,
    children,
    width = "32rem",
  }: {
    open?: boolean;
    title: string;
    subtitle?: string;
    children: Snippet;
    width?: string;
  } = $props();

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") open = false;
  }
</script>

<svelte:window {onkeydown} />

{#if open}
  <div
    class="fixed inset-0 z-[100] flex items-start justify-center bg-black/50 p-6 pt-[12vh] backdrop-blur-sm"
    role="presentation"
    onclick={(e) => {
      if (e.target === e.currentTarget) open = false;
    }}
  >
    <div
      class="card w-full p-5 shadow-2xl"
      style="max-width:{width}; background: var(--surface-solid)"
      role="dialog"
      aria-modal="true"
    >
      <h2 class="text-lg font-medium text-text">{title}</h2>
      {#if subtitle}
        <p class="mt-1 text-sm text-muted">{subtitle}</p>
      {/if}
      <div class="mt-4">
        {@render children()}
      </div>
    </div>
  </div>
{/if}
