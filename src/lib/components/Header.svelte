<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import Popover from "./Popover.svelte";
  import Dialog from "./Dialog.svelte";
  import SettingsDrawer from "./SettingsDrawer.svelte";
  import { autofocus } from "$lib/actions";
  import { inTauri } from "$lib/ipc";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    BookmarkSimple,
    ClipboardText,
    GearSix,
    Trash,
    FloppyDisk,
    ArrowsClockwise,
    GlobeHemisphereWest,
    Minus,
    Square,
    X,
  } from "phosphor-svelte";

  let importText = $state("");
  let saveName = $state("");

  // Custom window controls (native decorations are off). No-op in the browser
  // dev/mock path, where there's no Tauri window to drive. Resize is the fourth
  // piece of the same CSD, in ResizeGrips.svelte.
  const win = () => (inTauri ? getCurrentWindow() : null);

  function openSave() {
    saveName = app.activePresetName ?? app.selectedGameName ?? "";
    app.showSave = true;
  }
  function doSave() {
    if (!saveName.trim()) return;
    app.savePreset(saveName.trim());
    toast.success("Preset saved");
    app.showSave = false;
  }
  async function doRefresh() {
    await app.refresh();
    toast.success("Library refreshed");
  }
  async function doImport() {
    await app.importCommand(importText);
    toast.success("Imported");
    app.showImport = false;
    importText = "";
  }
</script>

<header data-tauri-drag-region class="flex items-center gap-2.5 px-4 py-2">
  <img src="/logo.svg" alt="" class="size-7 rounded-lg" data-tauri-drag-region />
  <div data-tauri-drag-region>
    <h1 class="text-sm font-medium leading-none text-text" data-tauri-drag-region>protongen</h1>
    {#if app.steamRoot}
      <p class="mt-0.5 text-[11px] leading-none text-muted" data-tauri-drag-region>{app.steamRoot}</p>
    {/if}
  </div>

  <div class="ml-auto flex items-center gap-1.5">
    {#if app.view === "builder"}
    <button
      onclick={() => (app.showImport = true)}
      class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50"
    >
      <ClipboardText size={14} /> Import
    </button>

    <!-- Presets -->
    <Popover width="16rem">
      {#snippet trigger({ props })}
        <button
          {...props}
          class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50"
        >
          <BookmarkSimple size={14} />
          {app.activePresetName ?? "Presets"}
        </button>
      {/snippet}
      <div class="space-y-1">
        {#if app.store.global_profile}
          <button
            onclick={() => {
              app.applyGlobalProfile();
              toast.success("Global profile applied");
            }}
            class="flex w-full items-center gap-1.5 rounded-lg border border-accent/40 px-2 py-1.5 text-xs font-medium text-accent hover:bg-accent/10"
          >
            <GlobeHemisphereWest size={13} /> Apply global profile
          </button>
          <div class="my-1 border-t border-border/60"></div>
        {/if}
        {#each app.store.presets as p (p.name)}
          <div class="flex items-center gap-1">
            <button
              onclick={() => app.loadPreset(p.name)}
              class="flex-1 truncate rounded-lg px-2 py-1.5 text-left text-sm text-subtext hover:bg-surface-2"
              >{p.name}</button
            >
            <button
              onclick={() => app.deletePreset(p.name)}
              class="grid size-7 place-items-center rounded-lg text-muted hover:text-red"
              aria-label="Delete preset"><Trash size={13} /></button
            >
          </div>
        {:else}
          <p class="px-2 py-3 text-center text-xs text-muted">No saved presets.</p>
        {/each}
        <button
          onclick={openSave}
          class="mt-1 flex w-full items-center gap-1.5 rounded-lg border border-border px-2 py-1.5 text-xs text-subtext hover:bg-surface-2"
        >
          <FloppyDisk size={13} /> Save current…
        </button>
      </div>
    </Popover>
    {/if}

    <!-- Refresh library (available on both library and builder views) -->
    <button
      onclick={doRefresh}
      disabled={app.refreshing}
      class="grid size-8 place-items-center rounded-lg border border-border bg-surface-2/50 text-subtext transition hover:border-accent/50 disabled:opacity-60"
      aria-label="Refresh library"
      title="Re-scan games, runtimes and shortcuts"
    >
      <ArrowsClockwise size={15} class={app.refreshing ? "animate-spin" : ""} />
    </button>

    <!-- Settings -->
    <button
      onclick={() => (app.showSettings = true)}
      class="grid size-8 place-items-center rounded-lg border border-border bg-surface-2/50 text-subtext transition hover:border-accent/50"
      aria-label="Settings"
    >
      <GearSix size={15} />
    </button>

    <!-- Window controls (native decorations are off) -->
    <div class="ml-1 flex items-center gap-1">
      <button
        onclick={() => win()?.minimize()}
        class="grid size-8 place-items-center rounded-lg text-muted transition hover:bg-surface-2 hover:text-text"
        aria-label="Minimize"
      >
        <Minus size={15} />
      </button>
      <button
        onclick={() => win()?.toggleMaximize()}
        class="grid size-8 place-items-center rounded-lg text-muted transition hover:bg-surface-2 hover:text-text"
        aria-label="Maximize"
      >
        <Square size={13} />
      </button>
      <button
        onclick={() => win()?.close()}
        class="grid size-8 place-items-center rounded-lg text-muted transition hover:bg-red hover:text-white"
        aria-label="Close"
      >
        <X size={15} />
      </button>
    </div>
  </div>
</header>

<Dialog
  bind:open={app.showImport}
  title="Import a command"
  subtitle="Paste a Steam launch-options string or a umu-run command."
>
  <textarea
    bind:value={importText}
    aria-label="Command to import"
    rows="3"
    placeholder="PROTON_USE_NTSYNC=1 mangohud %command%"
    class="w-full rounded-lg border border-border bg-surface-2 p-2.5 font-mono text-xs text-text outline-none focus:border-accent"
  ></textarea>
  <div class="mt-4 flex justify-end gap-2">
    <button onclick={() => (app.showImport = false)} class="rounded-lg px-3 py-1.5 text-sm text-muted hover:text-text"
      >Cancel</button
    >
    <button
      onclick={doImport}
      class="rounded-lg px-3 py-1.5 text-sm font-medium"
      style="background: var(--accent); color: var(--on-accent)">Parse &amp; fill</button
    >
  </div>
</Dialog>

<Dialog bind:open={app.showSave} title="Save preset" width="24rem">
  <input
    use:autofocus
    bind:value={saveName}
    aria-label="Preset name"
    placeholder="preset name"
    onkeydown={(e) => e.key === "Enter" && doSave()}
    class="w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm text-text outline-none focus:border-accent"
  />
  <div class="mt-4 flex justify-end gap-2">
    <button onclick={() => (app.showSave = false)} class="rounded-lg px-3 py-1.5 text-sm text-muted hover:text-text"
      >Cancel</button
    >
    <button
      onclick={doSave}
      disabled={!saveName.trim()}
      class="rounded-lg px-3 py-1.5 text-sm font-medium disabled:opacity-40"
      style="background: var(--accent); color: var(--on-accent)">Save</button
    >
  </div>
</Dialog>

<SettingsDrawer bind:open={app.showSettings} />
