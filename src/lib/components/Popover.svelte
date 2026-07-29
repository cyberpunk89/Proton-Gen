<script lang="ts">
  import type { Snippet } from "svelte";
  import { Popover as PopoverPrimitive } from "bits-ui";
  import { fly } from "$lib/motion.svelte";

  let {
    trigger,
    children,
    align = "end",
    width = "20rem",
    open = $bindable(false),
    trapFocus = true,
  }: {
    /**
     * Spread `props` onto your trigger element — bits-ui supplies the click
     * handler plus aria-expanded / aria-controls, which the hand-rolled
     * version never had.
     */
    trigger: Snippet<[{ props: Record<string, unknown>; open: boolean }]>;
    children: Snippet;
    align?: "start" | "end";
    width?: string;
    open?: boolean;
    /** Read-only bubbles should not steal focus; see InfoPopover. */
    trapFocus?: boolean;
  } = $props();
</script>

<!--
  Was `absolute top-full right-0 mt-2` with no collision handling, so the
  option-row info bubbles clipped inside the scroll container at App.svelte
  and RuntimePicker's tall list did the same. bits-ui positions with
  @floating-ui/dom, which flips and shifts to stay on screen.
-->
<PopoverPrimitive.Root bind:open>
  <PopoverPrimitive.Trigger>
    {#snippet child({ props })}
      {@render trigger({ props, open })}
    {/snippet}
  </PopoverPrimitive.Trigger>
  <PopoverPrimitive.Portal>
    <PopoverPrimitive.Content {align} sideOffset={8} {trapFocus} forceMount>
      <!--
        Floating content needs BOTH snippet arguments: wrapperProps carries the
        floating-ui positioning and props carries the content behaviour.
        Spreading only `props` renders a statically-positioned element on
        <body> — it looks fine and silently does not flip, which is the very
        bug this migration fixes.
      -->
      {#snippet child({ props, wrapperProps })}
        <div {...wrapperProps} class="z-50">
          {#if open}
            <div
              {...props}
              transition:fly={{ y: -4, duration: 120 }}
              class="popover p-3"
              style="width:{width}"
            >
              {@render children()}
            </div>
          {/if}
        </div>
      {/snippet}
    </PopoverPrimitive.Content>
  </PopoverPrimitive.Portal>
</PopoverPrimitive.Root>
