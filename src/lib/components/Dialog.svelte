<script lang="ts">
  import { Dialog as DialogPrimitive } from "bits-ui";
  import { X } from "phosphor-svelte";
  import { keys } from "$lib/keys.svelte";
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

  // Tell the global key layer an overlay is up, so single-key bindings ("/",
  // "?", Escape-to-library) stay quiet underneath it. bits-ui already layers
  // Escape correctly among its own overlays; this covers *our* global handler,
  // which it knows nothing about.
  $effect(() => {
    if (!open) return;
    keys.pushOverlay();
    return () => keys.popOverlay();
  });
</script>

<!--
  bits-ui supplies what the hand-rolled version lacked: a focus trap, focus
  restore to the trigger on close, and — the actual bug fix — layered Escape
  handling via globalThis.bitsEscapeLayers, so only the top-most overlay
  responds. The old svelte:window handler fired regardless of what was on top,
  which meant one Escape closed both this dialog and the Settings drawer
  underneath it.

  Markup, classes and the prop surface are unchanged, so no caller changes.
-->
<DialogPrimitive.Root bind:open>
  <DialogPrimitive.Portal>
    <DialogPrimitive.Overlay
      class="fixed inset-0 z-[100] bg-black/50 backdrop-blur-sm"
    />
    <DialogPrimitive.Content>
      {#snippet child({ props })}
        <div
          class="fixed inset-0 z-[100] flex items-start justify-center p-6 pt-[12vh]"
          role="presentation"
        >
          <div {...props} class="card w-full p-5 shadow-2xl" style="max-width:{width}; background: var(--surface-solid)">
            <div class="flex items-start gap-3">
              <div class="min-w-0 flex-1">
                <DialogPrimitive.Title class="text-lg font-medium text-text">
                  {title}
                </DialogPrimitive.Title>
                {#if subtitle}
                  <DialogPrimitive.Description class="mt-1 text-sm text-muted">
                    {subtitle}
                  </DialogPrimitive.Description>
                {/if}
              </div>
              <DialogPrimitive.Close
                class="-mr-1 -mt-1 shrink-0 rounded-lg p-1 text-muted transition hover:text-text"
                aria-label="Close dialog"
              >
                <X size={16} />
              </DialogPrimitive.Close>
            </div>
            <div class="mt-4">
              {@render children()}
            </div>
          </div>
        </div>
      {/snippet}
    </DialogPrimitive.Content>
  </DialogPrimitive.Portal>
</DialogPrimitive.Root>
