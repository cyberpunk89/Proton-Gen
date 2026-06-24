<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import { fade, slide } from "svelte/transition";
  import GamePicker from "./GamePicker.svelte";
  import RuntimePicker from "./RuntimePicker.svelte";
  import ModeToggle from "./ModeToggle.svelte";
  import CommandPreview from "./CommandPreview.svelte";
  import UmuFields from "./UmuFields.svelte";
  import ProtonDbChip from "./ProtonDbChip.svelte";
  import { DownloadSimple, CaretRight, SlidersHorizontal } from "phosphor-svelte";

  let selectedGame = $derived(
    app.selectedAppId == null
      ? null
      : app.games.find((g) => g.app_id === app.selectedAppId) ?? null,
  );
  let isSteam = $derived(selectedGame?.source === "steam");

  // Wide banner art for the selected game: prefer hero, fall back to header.
  let bannerArt = $derived(
    selectedGame
      ? app.artFor(selectedGame.app_id, selectedGame.source, "hero") ??
          app.artFor(selectedGame.app_id, selectedGame.source, "header") ??
          null
      : null,
  );

  $effect(() => {
    if (selectedGame) {
      app.requestArt(selectedGame.app_id, selectedGame.source, "hero");
      app.requestArt(selectedGame.app_id, selectedGame.source, "header");
    }
  });

  // Proton runtime + launch args + custom env are rarely changed — keep them
  // tucked away. Force-open when in umu mode (PROTONPATH matters) or when a
  // selected preset/game already populated args/env, so nothing hides silently.
  let advancedOpen = $state(false);
  let hasAdvancedContent = $derived(
    app.gameArgs.trim() !== "" || app.extraEnv.trim() !== "",
  );
  let showAdvanced = $derived(advancedOpen || app.umu || hasAdvancedContent);
  let currentOpts = $derived(
    isSteam && app.selectedAppId != null
      ? app.launchOptions[String(app.selectedAppId)] ?? ""
      : "",
  );
  let currentTool = $derived(
    isSteam && app.selectedAppId != null
      ? app.compatTools[String(app.selectedAppId)] ?? ""
      : "",
  );

  async function loadCurrent() {
    if (!currentOpts) return;
    await app.importCommand(currentOpts);
    toast.show("Loaded current launch options");
  }
</script>

<section class="card relative overflow-hidden p-4">
  {#if bannerArt}
    <div class="relative -mx-4 -mt-4 mb-4 h-28 overflow-hidden" transition:fade={{ duration: 220 }}>
      <img src={bannerArt} alt="" class="h-full w-full object-cover object-center" />
      <div
        class="absolute inset-0"
        style="background: linear-gradient(to top, var(--surface-solid) 3%, color-mix(in srgb, var(--surface-solid) 25%, transparent) 55%, transparent)"
      ></div>
      <p
        class="absolute bottom-2 left-4 right-4 truncate text-lg font-semibold text-text"
        style="text-shadow: 0 1px 6px rgb(0 0 0 / 60%)"
      >
        {app.selectedGameName}
      </p>
    </div>
  {/if}

  <div class="flex flex-col gap-3 sm:flex-row sm:items-stretch">
    <div class="flex-1"><GamePicker /></div>
    <div class="flex items-end"><ModeToggle /></div>
  </div>

  {#if isSteam}
    <div class="mt-3 flex flex-wrap items-center gap-3">
      <ProtonDbChip />
      {#if currentOpts || currentTool}
        <div class="ml-auto flex items-center gap-2 text-xs text-muted">
          {#if currentTool}
            <span class="font-mono">Proton={currentTool}</span>
          {/if}
          {#if currentOpts}
            <span class="max-w-[280px] truncate font-mono" title={currentOpts}
              >{currentOpts}</span
            >
            <button
              onclick={loadCurrent}
              class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/60 px-2.5 py-1.5 text-subtext transition hover:border-accent/50"
            >
              <DownloadSimple size={13} /> Load current
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  {#if app.umu}
    <div class="mt-4"><UmuFields /></div>
  {/if}

  <!-- Advanced: Proton runtime + launch args + custom env (hidden until revealed) -->
  <button
    type="button"
    onclick={() => (advancedOpen = !advancedOpen)}
    class="mt-3 flex w-full items-center gap-2 rounded-lg px-1 py-1.5 text-xs text-muted transition hover:text-subtext"
  >
    <SlidersHorizontal size={14} />
    <span class="font-medium">Advanced</span>
    {#if app.selectedRuntime}
      <span class="max-w-[280px] truncate">· Proton: {app.selectedRuntime.display_name}</span>
    {/if}
    {#if hasAdvancedContent}
      <span class="text-accent">· customized</span>
    {/if}
    <CaretRight
      size={14}
      class="ml-auto shrink-0 transition-transform {showAdvanced ? 'rotate-90' : ''}"
    />
  </button>

  {#if showAdvanced}
    <div transition:slide={{ duration: 180 }} class="mt-1 space-y-3">
      <RuntimePicker />
      <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <label class="block">
          <span class="mb-1 block text-[11px] uppercase tracking-wider text-muted"
            >Game arguments</span
          >
          <input
            bind:value={app.gameArgs}
            placeholder="-windowed -novid"
            class="w-full rounded-lg border border-border bg-surface-2 px-2.5 py-2 font-mono text-xs text-text outline-none focus:border-accent"
          />
        </label>
        <label class="block">
          <span class="mb-1 block text-[11px] uppercase tracking-wider text-muted"
            >Custom env</span
          >
          <input
            bind:value={app.extraEnv}
            placeholder="KEY=VALUE KEY2=VALUE2"
            class="w-full rounded-lg border border-border bg-surface-2 px-2.5 py-2 font-mono text-xs text-text outline-none focus:border-accent"
          />
        </label>
      </div>
    </div>
  {/if}

  <div class="mt-4"><CommandPreview /></div>
</section>
