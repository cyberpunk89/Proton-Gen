<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import Popover from "./Popover.svelte";
  import Dialog from "./Dialog.svelte";
  import SettingsDrawer from "./SettingsDrawer.svelte";
  import { autofocus } from "$lib/actions";
  import {
    BookmarkSimple,
    ClipboardText,
    GearSix,
    Trash,
    FloppyDisk,
  } from "phosphor-svelte";

  let showImport = $state(false);
  let showSave = $state(false);
  let showSettings = $state(false);
  let importText = $state("");
  let saveName = $state("");

  function openSave() {
    saveName = app.activePresetName ?? app.selectedGameName ?? "";
    showSave = true;
  }
  function doSave() {
    if (!saveName.trim()) return;
    app.savePreset(saveName.trim());
    toast.show("Preset saved");
    showSave = false;
  }
  async function doImport() {
    await app.importCommand(importText);
    toast.show("Imported");
    showImport = false;
    importText = "";
  }
</script>

<header class="flex items-center gap-3 px-5 py-3">
  <img src="/logo.svg" alt="" class="size-8 rounded-lg" />
  <div>
    <h1 class="text-base font-medium leading-none text-text">protongen</h1>
    {#if app.steamRoot}
      <p class="mt-0.5 text-[11px] text-muted">{app.steamRoot}</p>
    {/if}
  </div>

  <div class="ml-auto flex items-center gap-1.5">
    <button
      onclick={() => (showImport = true)}
      class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50"
    >
      <ClipboardText size={14} /> Import
    </button>

    <!-- Presets -->
    <Popover width="16rem">
      {#snippet trigger({ toggle })}
        <button
          onclick={toggle}
          class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50"
        >
          <BookmarkSimple size={14} />
          {app.activePresetName ?? "Presets"}
        </button>
      {/snippet}
      <div class="space-y-1">
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

    <!-- Settings -->
    <button
      onclick={() => (showSettings = true)}
      class="grid size-8 place-items-center rounded-lg border border-border bg-surface-2/50 text-subtext transition hover:border-accent/50"
      aria-label="Settings"
    >
      <GearSix size={15} />
    </button>
  </div>
</header>

<Dialog
  bind:open={showImport}
  title="Import a command"
  subtitle="Paste a Steam launch-options string or a umu-run command."
>
  <textarea
    bind:value={importText}
    rows="3"
    placeholder="PROTON_USE_NTSYNC=1 mangohud %command%"
    class="w-full rounded-lg border border-border bg-surface-2 p-2.5 font-mono text-xs text-text outline-none focus:border-accent"
  ></textarea>
  <div class="mt-4 flex justify-end gap-2">
    <button onclick={() => (showImport = false)} class="rounded-lg px-3 py-1.5 text-sm text-muted hover:text-text"
      >Cancel</button
    >
    <button
      onclick={doImport}
      class="rounded-lg px-3 py-1.5 text-sm font-medium"
      style="background: var(--accent); color: var(--on-accent)">Parse &amp; fill</button
    >
  </div>
</Dialog>

<Dialog bind:open={showSave} title="Save preset" width="24rem">
  <input
    use:autofocus
    bind:value={saveName}
    placeholder="preset name"
    onkeydown={(e) => e.key === "Enter" && doSave()}
    class="w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm text-text outline-none focus:border-accent"
  />
  <div class="mt-4 flex justify-end gap-2">
    <button onclick={() => (showSave = false)} class="rounded-lg px-3 py-1.5 text-sm text-muted hover:text-text"
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

<SettingsDrawer bind:open={showSettings} />
