<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import { copyText } from "$lib/util";
  import { encodePreset } from "$lib/presetCode";
  import { autofocus } from "$lib/actions";
  import type { Preset } from "$lib/types";
  import { PencilSimple, ShareNetwork, Trash, Check } from "phosphor-svelte";

  let { preset }: { preset: Preset } = $props();

  let editing = $state(false);
  let draft = $state("");
  let error = $state<string | null>(null);

  function startRename() {
    draft = preset.name;
    error = null;
    editing = true;
  }

  function commitRename() {
    error = app.renamePreset(preset.name, draft);
    if (!error) editing = false;
  }

  async function share() {
    await copyText(encodePreset({ name: preset.name, config: preset.config }));
    toast.success(`Share code for “${preset.name}” copied — paste it into Import on another machine`);
  }

  function remove() {
    const removed = app.deletePreset(preset.name);
    if (!removed) return;
    toast.success(`Deleted “${removed.preset.name}”`, {
      action: { label: "Undo", onClick: () => app.restorePreset(removed.preset, removed.index) },
    });
  }
</script>

{#if editing}
  <div class="space-y-1 px-1 py-0.5">
    <div class="flex items-center gap-1">
      <input
        use:autofocus
        bind:value={draft}
        aria-label="New preset name"
        aria-invalid={error != null}
        onkeydown={(e) => {
          if (e.key === "Enter") commitRename();
          if (e.key === "Escape") {
            // Cancel the rename, not the whole popover.
            e.stopPropagation();
            editing = false;
          }
        }}
        class="min-w-0 flex-1 rounded-md border border-border bg-surface-2 px-2 py-1 text-sm text-text outline-none focus:border-accent"
      />
      <button
        onclick={commitRename}
        class="grid size-7 place-items-center rounded-lg text-muted hover:text-accent"
        aria-label="Save name"><Check size={13} /></button
      >
    </div>
    {#if error}<p class="px-1 text-[11px] text-red">{error}</p>{/if}
  </div>
{:else}
  <div class="group flex items-center gap-0.5">
    <button
      onclick={() => app.loadPreset(preset.name)}
      class="flex-1 truncate rounded-lg px-2 py-1.5 text-left text-sm text-subtext hover:bg-surface-2"
      >{preset.name}</button
    >
    <button
      onclick={startRename}
      class="grid size-7 place-items-center rounded-lg text-muted hover:text-text"
      title="Rename"
      aria-label="Rename preset {preset.name}"><PencilSimple size={13} /></button
    >
    <button
      onclick={share}
      class="grid size-7 place-items-center rounded-lg text-muted hover:text-text"
      title="Copy share code"
      aria-label="Copy share code for {preset.name}"><ShareNetwork size={13} /></button
    >
    <button
      onclick={remove}
      class="grid size-7 place-items-center rounded-lg text-muted hover:text-red"
      title="Delete"
      aria-label="Delete preset {preset.name}"><Trash size={13} /></button
    >
  </div>
{/if}
