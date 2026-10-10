<script lang="ts">
  import { untrack } from "svelte";
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import { droppedNote } from "$lib/util";
  import Popover from "./Popover.svelte";
  import Dialog from "./Dialog.svelte";
  import Lazy from "./Lazy.svelte";
  import { lazy } from "$lib/lazy";
  import UiModeToggle from "./UiModeToggle.svelte";
  import PresetRow from "./PresetRow.svelte";
  import { decodePreset, isPresetCode } from "$lib/presetCode";
  import type { SharedPreset } from "$lib/presetCode";
  import { readText } from "@tauri-apps/plugin-clipboard-manager";
  import { autofocus } from "$lib/actions";
  import { inTauri } from "$lib/ipc";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    ArrowLeft,
    BookmarkSimple,
    ClipboardText,
    GearSix,
    FloppyDisk,
    ArrowsLeftRight,
    ArrowsClockwise,
    GlobeHemisphereWest,
    FileText,
    Robot,
    Minus,
    Square,
    X,
  } from "phosphor-svelte";

  let importText = $state("");
  let importError = $state<string | null>(null);
  let importing = $state(false);
  let saveName = $state("");
  /** Saving over a preset that isn't the loaded one takes a second press. */
  let overwriteArmed = $state(false);

  let trimmedSave = $derived(saveName.trim());
  let saveTarget = $derived(app.store.presets.find((p) => p.name === trimmedSave) ?? null);
  let saveIsActive = $derived(saveTarget != null && trimmedSave === app.activePresetName);

  // Seed the name whenever the dialog opens — from the popover or the palette.
  $effect(() => {
    if (!app.showSave) return;
    untrack(() => {
      saveName = app.activePresetName ?? app.selectedGameName ?? "";
      overwriteArmed = false;
    });
  });

  // A pasted share code turns the Import dialog into a preset preview.
  let shared = $derived.by(() => {
    if (!isPresetCode(importText)) return null;
    return decodePreset(importText, app.catalog.wrappers.map((w) => w.key));
  });
  let sharedName = $state("");
  let sharedPreset = $derived<SharedPreset | null>(shared?.ok ? shared.preset : null);
  $effect(() => {
    const p = sharedPreset;
    untrack(() => (sharedName = p?.name ?? ""));
  });
  /** Game args and gamescope args go on the command line verbatim, so a shared
   *  preset's are worth a second look before use. */
  let sharedShellSyntax = $derived.by(() => {
    const p = sharedPreset;
    if (!p) return false;
    const verbatim = [p.config.game_args, ...p.config.wrappers.map(([, v]) => v)];
    return verbatim.some((v) => /[;&|`<>]|\$\(/.test(v));
  });

  // Custom window controls (native decorations are off). No-op in the browser
  // dev/mock path, where there's no Tauri window to drive. Resize is the fourth
  // piece of the same CSD, in ResizeGrips.svelte.
  const win = () => (inTauri ? getCurrentWindow() : null);

  function openSave() {
    app.showSave = true;
  }
  function doSave() {
    if (!trimmedSave) return;
    if (saveTarget && !saveIsActive && !overwriteArmed) {
      overwriteArmed = true;
      return;
    }
    app.savePreset(trimmedSave);
    toast.success(saveTarget ? `Preset “${trimmedSave}” updated` : "Preset saved");
    app.showSave = false;
  }
  function saveShared(load: boolean) {
    const p = sharedPreset;
    const name = sharedName.trim();
    if (!p || !name) return;
    app.addSharedPreset({ name, config: p.config }, load);
    toast.success(load ? `Imported and loaded “${name}”` : `Imported preset “${name}”`);
    app.showImport = false;
    importText = "";
  }
  async function pasteFromClipboard() {
    try {
      importText = inTauri ? await readText() : await navigator.clipboard.readText();
      importError = null;
    } catch {
      toast.error("Couldn't read the clipboard");
    }
  }
  async function doRefresh() {
    const result = await app.refresh();
    if (result === "ok") toast.success("Library refreshed");
    else if (result === "failed") toast.error("Couldn't refresh the library");
  }
  async function doImport() {
    if (!importText.trim() || importing) return;
    importing = true;
    importError = null;
    let dropped: string[];
    try {
      dropped = await app.importCommand(importText);
    } catch (e) {
      // Kept open with the text intact, so a typo can be fixed in place.
      importError = e instanceof Error ? e.message : String(e);
      return;
    } finally {
      importing = false;
    }
    toast.success(`Imported${droppedNote(dropped)}`, { action: { label: "Undo", onClick: () => app.undo() } });
    app.showImport = false;
    importText = "";
  }
</script>

<header data-tauri-drag-region class="relative flex items-center gap-2.5 px-4 py-2">
  <!-- Accent hairline along the bottom edge, fading out at both ends. -->
  <span
    class="pointer-events-none absolute inset-x-0 bottom-0 h-px"
    style="background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--accent) 45%, transparent) 30%, color-mix(in srgb, var(--mauve) 45%, transparent) 70%, transparent)"
    aria-hidden="true"
  ></span>
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
      onclick={() => app.backToLibrary()}
      title="Back to library"
      aria-label="Back to library"
      class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50"
    >
      <ArrowLeft size={14} /> Library
    </button>

    <UiModeToggle />

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
          {#if app.presetModified}
            <span class="text-peach" title="Changed since this preset was loaded" aria-label="modified"
              >•</span
            >
          {/if}
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
        {#if app.presetsForCurrentGame.length}
          <p class="px-2 pb-0.5 text-[10px] font-medium uppercase tracking-wide text-muted">For this game</p>
          {#each app.presetsForCurrentGame as p (p.name)}
            <PresetRow preset={p} />
          {/each}
          <div class="my-1 border-t border-border/60"></div>
        {/if}
        {#if app.otherPresets.length}
          {#if app.presetsForCurrentGame.length}
            <p class="px-2 pb-0.5 text-[10px] font-medium uppercase tracking-wide text-muted">Other presets</p>
          {/if}
          {#each app.otherPresets as p (p.name)}
            <PresetRow preset={p} />
          {/each}
        {/if}
        {#if !app.presetsForCurrentGame.length && !app.otherPresets.length}
          <p class="px-2 py-3 text-center text-xs text-muted">No saved presets.</p>
        {/if}
        <button
          onclick={openSave}
          class="mt-1 flex w-full items-center gap-1.5 rounded-lg border border-border px-2 py-1.5 text-xs text-subtext hover:bg-surface-2"
        >
          <FloppyDisk size={13} /> Save current…
        </button>
        <button
          onclick={() => (app.compareOpen = true)}
          class="flex w-full items-center gap-1.5 rounded-lg border border-border px-2 py-1.5 text-xs text-subtext hover:bg-surface-2"
        >
          <ArrowsLeftRight size={13} /> Compare…
        </button>
      </div>
    </Popover>

    <!-- Per-game Proton log viewer. Only meaningful with a game selected. -->
    <button
      onclick={() => (app.showLogs = true)}
      disabled={app.selectedAppId == null}
      class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50 disabled:opacity-50"
      title={app.selectedAppId == null ? "Select a game to view its Proton log" : "View this game's Proton log"}
    >
      <FileText size={14} /> Logs
    </button>

    <!-- AI symptom troubleshooter. Opt-in; only shown when the AI coach is on. -->
    {#if app.store.llm_enabled}
      <button
        onclick={() => (app.showTroubleshooter = true)}
        class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50"
        title="Describe a problem and get AI-suggested fixes"
      >
        <Robot size={14} /> Troubleshoot
      </button>
    {/if}
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
        onclick={() => void app.flushPersist().finally(() => win()?.close())}
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
    oninput={() => (importError = null)}
    aria-label="Command to import"
    aria-invalid={importError != null}
    aria-describedby={importError ? "import-error" : undefined}
    rows="3"
    placeholder="PROTON_USE_NTSYNC=1 mangohud %command%"
    class="w-full rounded-lg border border-border bg-surface-2 p-2.5 font-mono text-xs text-text outline-none focus:border-accent"
  ></textarea>
  {#if importError}
    <p id="import-error" role="alert" class="mt-2 text-xs text-red">{importError}</p>
  {/if}
  {#if shared && !shared.ok}
    <p role="alert" class="mt-2 text-xs text-red">{shared.error}</p>
  {:else if shared?.ok && sharedPreset}
    <!-- A share code: preview what it will add before it touches anything. -->
    <div class="mt-3 space-y-2 rounded-lg border border-border/60 bg-surface-2/40 p-3 text-xs">
      <p class="font-medium text-text">Preset from a share code</p>
      <label class="block">
        <span class="text-muted">Save as</span>
        <input
          bind:value={sharedName}
          aria-label="Name for the imported preset"
          class="mt-1 w-full rounded-md border border-border bg-surface-2 px-2 py-1 text-sm text-text outline-none focus:border-accent"
        />
      </label>
      {#if app.presetExists(sharedName.trim())}
        <p class="text-peach">A preset called “{sharedName.trim()}” exists — saving replaces it.</p>
      {/if}
      <pre class="max-h-32 overflow-auto whitespace-pre-wrap break-words font-mono text-[11px] text-subtext">{[
          ...sharedPreset.config.env.map(([k, v]) => `${k}=${v}`),
          ...sharedPreset.config.wrappers.map(([k, v]) => (v ? `${k} ${v}` : k)),
          sharedPreset.config.extra_env,
          sharedPreset.config.game_args && `game args: ${sharedPreset.config.game_args}`,
        ]
          .filter(Boolean)
          .join("\n") || "(empty preset)"}</pre>
      {#if sharedShellSyntax}
        <p class="text-peach">
          Its game or gamescope arguments contain shell syntax, which runs as-is when
          Steam launches the game. Check it's what you expect.
        </p>
      {/if}
      {#if shared.droppedWrappers.length}
        <p class="text-muted">Skipped unknown wrappers: {shared.droppedWrappers.join(", ")}.</p>
      {/if}
    </div>
  {/if}
  <div class="mt-4 flex items-center justify-end gap-2">
    <button
      onclick={pasteFromClipboard}
      class="mr-auto inline-flex items-center gap-1 rounded-lg px-2 py-1.5 text-xs text-muted hover:text-text"
    >
      <ClipboardText size={13} /> Paste from clipboard
    </button>
    <button onclick={() => (app.showImport = false)} class="rounded-lg px-3 py-1.5 text-sm text-muted hover:text-text"
      >Cancel</button
    >
    {#if sharedPreset}
      <button
        onclick={() => saveShared(false)}
        disabled={!sharedName.trim()}
        class="rounded-lg border border-border px-3 py-1.5 text-sm text-subtext hover:border-accent/50 disabled:opacity-50"
        >Save</button
      >
      <button
        onclick={() => saveShared(true)}
        disabled={!sharedName.trim()}
        class="rounded-lg px-3 py-1.5 text-sm font-medium disabled:opacity-50"
        style="background: var(--accent); color: var(--on-accent)">Save &amp; load</button
      >
    {:else}
      <button
        onclick={doImport}
        disabled={!importText.trim() || importing || shared != null}
        class="rounded-lg px-3 py-1.5 text-sm font-medium disabled:cursor-not-allowed disabled:opacity-50"
        style="background: var(--accent); color: var(--on-accent)">Parse &amp; fill</button
      >
    {/if}
  </div>
</Dialog>

<Dialog bind:open={app.showSave} title="Save preset" width="24rem">
  <input
    use:autofocus
    bind:value={saveName}
    aria-label="Preset name"
    placeholder="preset name"
    oninput={() => (overwriteArmed = false)}
    onkeydown={(e) => e.key === "Enter" && doSave()}
    class="w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm text-text outline-none focus:border-accent"
  />
  {#if saveTarget && !saveIsActive}
    <p role="alert" class="mt-2 text-xs text-peach">
      A preset called “{trimmedSave}” already exists{saveTarget.game_name
        ? ` (for ${saveTarget.game_name})`
        : ""}.
      {overwriteArmed ? "Press Overwrite again to replace it." : "Saving replaces it."}
    </p>
  {/if}
  <div class="mt-4 flex justify-end gap-2">
    <button onclick={() => (app.showSave = false)} class="rounded-lg px-3 py-1.5 text-sm text-muted hover:text-text"
      >Cancel</button
    >
    <button
      onclick={doSave}
      disabled={!trimmedSave}
      class="rounded-lg px-3 py-1.5 text-sm font-medium disabled:opacity-40"
      style="background: {saveTarget && !saveIsActive ? 'var(--peach)' : 'var(--accent)'}; color: var(--on-accent)"
      >{saveIsActive ? "Update" : saveTarget ? "Overwrite" : "Save"}</button
    >
  </div>
</Dialog>

<Lazy load={lazy.settings} when={app.showSettings}>
  {#snippet children(C)}<C bind:open={app.showSettings} />{/snippet}
</Lazy>
