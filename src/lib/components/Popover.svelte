<script lang="ts">
  import type { Snippet } from "svelte";
  import { fly } from "svelte/transition";
  import { clickOutside } from "$lib/actions";

  let {
    trigger,
    children,
    align = "end",
    width = "20rem",
    open = $bindable(false),
  }: {
    trigger: Snippet<[{ toggle: () => void; open: boolean }]>;
    children: Snippet;
    align?: "start" | "end";
    width?: string;
    open?: boolean;
  } = $props();

  function toggle() {
    open = !open;
  }
</script>

<span class="relative inline-flex">
  {@render trigger({ toggle, open })}
  {#if open}
    <div
      use:clickOutside={() => (open = false)}
      transition:fly={{ y: -4, duration: 120 }}
      class="popover absolute top-full z-50 mt-2 p-3 {align === 'end'
        ? 'right-0'
        : 'left-0'}"
      style="width:{width}"
    >
      {@render children()}
    </div>
  {/if}
</span>
