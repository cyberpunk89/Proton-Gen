<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { THEMES } from "$lib/themes";
  import { fly, fade } from "svelte/transition";
  import Switch from "./Switch.svelte";
  import MangoHud from "./MangoHud.svelte";
  import { GearSix, Palette, Gauge, SlidersHorizontal, Check, X } from "phosphor-svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") open = false;
  }
</script>

<svelte:window {onkeydown} />

{#if open}
  <div
    class="fixed inset-0 z-[100] flex justify-end bg-black/50 backdrop-blur-sm"
    role="presentation"
    transition:fade={{ duration: 120 }}
    onclick={(e) => {
      if (e.target === e.currentTarget) open = false;
    }}
  >
    <div
      class="flex h-full w-[360px] max-w-[90vw] flex-col border-l border-border shadow-2xl"
      style="background: var(--surface-solid)"
      transition:fly={{ x: 360, duration: 200 }}
      role="dialog"
      aria-modal="true"
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
          <h3
            class="mb-2 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted"
          >
            <Palette size={13} /> Appearance
          </h3>
          <div class="grid grid-cols-2 gap-1.5">
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
        </section>

        <!-- Behavior -->
        <section>
          <h3
            class="mb-2 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted"
          >
            <SlidersHorizontal size={13} /> Behavior
          </h3>
          <div class="space-y-0.5">
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
        </section>

        <!-- Overlay -->
        <section>
          <h3
            class="mb-2 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted"
          >
            <Gauge size={13} /> MangoHud overlay
          </h3>
          <MangoHud />
        </section>
      </div>
    </div>
  </div>
{/if}

{#snippet toggle(title: string, desc: string, checked: boolean, onchange: () => void)}
  <div class="flex items-center gap-3 rounded-lg px-1 py-1.5">
    <div class="min-w-0 flex-1">
      <p class="text-sm text-subtext">{title}</p>
      <p class="text-[11px] leading-snug text-muted">{desc}</p>
    </div>
    <Switch {checked} {onchange} label={title} />
  </div>
{/snippet}
