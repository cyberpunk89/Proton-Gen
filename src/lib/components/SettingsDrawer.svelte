<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { keys } from "$lib/keys.svelte";
  import { THEMES } from "$lib/themes";
  import { fly, fade } from "$lib/motion.svelte";
  import { mergeStyle } from "$lib/util";
  import { Dialog as DialogPrimitive } from "bits-ui";
  import Switch from "./Switch.svelte";
  import { GearSix, Palette, SlidersHorizontal, Check, X, CaretDown } from "phosphor-svelte";
  import type { Component } from "svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  // Keep the global key layer quiet while the drawer is up — see Dialog.svelte.
  $effect(() => {
    if (!open) return;
    keys.pushOverlay();
    return () => keys.popOverlay();
  });

  // Sections are collapsible; all start collapsed to keep the drawer tidy.
  let sections = $state({ appearance: false, behavior: false });
</script>

<!--
  The bespoke fly/fade markup is kept verbatim; only the layer behaviour
  (escape stack, focus trap, focus restore, scroll lock) now comes from
  bits-ui. forceMount + an inner {#if open} is what preserves the exit
  transition, since bits-ui unmounts on close by default.

  This drawer used to register its own competing svelte:window Escape handler,
  which is half of the layering bug: with the Save-preset dialog open on top,
  one Escape closed both.
-->
<DialogPrimitive.Root bind:open>
  <DialogPrimitive.Portal>
    <DialogPrimitive.Overlay forceMount>
      {#snippet child({ props })}
        {#if open}
          <div
            {...props}
            class="fixed inset-0 z-[100] bg-black/50 backdrop-blur-sm"
            transition:fade={{ duration: 120 }}
          ></div>
        {/if}
      {/snippet}
    </DialogPrimitive.Overlay>
    <DialogPrimitive.Content forceMount>
      {#snippet child({ props })}
        {#if open}
          <div class="fixed inset-0 z-[100] flex justify-end" role="presentation">
            <div
              {...props}
              class="flex h-full w-[360px] max-w-[90vw] flex-col border-l border-border shadow-2xl"
              style={mergeStyle(props, "background: var(--surface-solid)")}
              transition:fly={{ x: 360, duration: 200 }}
              aria-label="Settings"
            >
      <header class="flex items-center gap-2 border-b border-border px-4 py-3">
        <GearSix size={18} weight="fill" class="text-accent" />
        <h2 class="text-sm font-medium text-text">Settings</h2>
        <button
          onclick={() => (open = false)}
          aria-label="Close settings"
          class="ml-auto grid size-7 place-items-center rounded-lg text-muted transition hover:bg-surface-2 hover:text-text"
        >
          <X size={16} />
        </button>
      </header>

      <div class="flex-1 space-y-6 overflow-y-auto p-4">
        <!-- Appearance -->
        <section>
          {@render sectionHeading(
            Palette,
            "Appearance",
            sections.appearance,
            () => (sections.appearance = !sections.appearance),
          )}
          {#if sections.appearance}
          <div id="drawer-section-appearance" class="mt-2 grid grid-cols-2 gap-1.5">
            {#each THEMES as t (t.id)}
              <button
                onclick={() => app.setTheme(t.id)}
                class="flex items-center gap-1.5 rounded-lg border px-2.5 py-2 text-left text-xs transition {app
                  .store.theme === t.id
                  ? 'border-accent text-text'
                  : 'border-border text-subtext hover:border-accent/50'}"
              >
                {#if app.store.theme === t.id}
                  <Check size={12} class="shrink-0 text-accent" />
                {:else}
                  <span class="size-3 shrink-0"></span>
                {/if}
                <span class="truncate">{t.label}</span>
              </button>
            {/each}
          </div>
          {/if}
        </section>

        <!-- Behavior -->
        <section>
          {@render sectionHeading(
            SlidersHorizontal,
            "Behavior",
            sections.behavior,
            () => (sections.behavior = !sections.behavior),
          )}
          {#if sections.behavior}
          <div id="drawer-section-behavior" class="mt-2 space-y-0.5">
            {@render toggle(
              "Show unsupported options",
              "List recipes that don't match your detected hardware.",
              app.store.show_irrelevant,
              () => app.setShowIrrelevant(!app.store.show_irrelevant),
            )}
            {@render toggle(
              "I have an HDR display",
              "Enables HDR recipes. HDR can't be auto-detected.",
              app.store.hdr,
              () => app.setHdr(!app.store.hdr),
            )}
            {@render toggle(
              "I have an RDNA3/RDNA4 GPU",
              "Shows FSR 3/4 upscaler-upgrade options (hidden by default).",
              app.store.fsr4,
              () => app.setFsr4(!app.store.fsr4),
            )}
            {@render toggle(
              "Auto-check ProtonDB",
              "Fetch the compatibility tier when a Steam game is selected.",
              app.store.protondb_auto,
              () => app.setProtondbAuto(!app.store.protondb_auto),
            )}
          </div>
          {/if}
        </section>
      </div>
            </div>
          </div>
        {/if}
      {/snippet}
    </DialogPrimitive.Content>
  </DialogPrimitive.Portal>
</DialogPrimitive.Root>

<!-- One fix covers all three drawer sections, since they share this snippet. -->
{#snippet sectionHeading(Icon: Component, label: string, isOpen: boolean, onclick: () => void)}
  <button
    {onclick}
    aria-expanded={isOpen}
    aria-controls={`drawer-section-${label.replace(/\W+/g, "-").toLowerCase()}`}
    class="flex w-full items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted transition hover:text-subtext"
  >
    <Icon size={13} />
    {label}
    <CaretDown size={12} class="ml-auto transition-transform {isOpen ? '' : '-rotate-90'}" />
  </button>
{/snippet}

{#snippet toggle(title: string, desc: string, checked: boolean, onchange: () => void)}
  {@const id = `setting-${title.replace(/\W+/g, "-").toLowerCase()}`}
  <div class="flex items-center gap-3 rounded-lg px-1 py-1.5">
    <div class="min-w-0 flex-1">
      <p {id} class="text-sm text-subtext">{title}</p>
      <p class="text-[11px] leading-snug text-muted">{desc}</p>
    </div>
    <Switch {checked} {onchange} labelledby={id} />
  </div>
{/snippet}
