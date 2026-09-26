<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "$lib/motion.svelte";
  import { app } from "$lib/state.svelte";
  import { keys } from "$lib/keys.svelte";
  import Header from "$lib/components/Header.svelte";
  import NavRail from "$lib/components/NavRail.svelte";
  import MainPanel from "$lib/components/MainPanel.svelte";
  import SimplePanel from "$lib/components/SimplePanel.svelte";
  import Library from "$lib/components/Library.svelte";
  import CommandPreview from "$lib/components/CommandPreview.svelte";
  import Notices from "$lib/components/Notices.svelte";
  import StaleBanner from "$lib/components/StaleBanner.svelte";
  import UpdateBanner from "$lib/components/UpdateBanner.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import ShortcutsSheet from "$lib/components/ShortcutsSheet.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import HeroicConfirm from "$lib/components/HeroicConfirm.svelte";
  import MangoHudSystemConfirm from "$lib/components/MangoHudSystemConfirm.svelte";
  import VkBasaltSystemConfirm from "$lib/components/VkBasaltSystemConfirm.svelte";
  import OverlayBuilders from "$lib/components/OverlayBuilders.svelte";
  import DefaultProfilePrompt from "$lib/components/DefaultProfilePrompt.svelte";
  import IntroTour from "$lib/components/IntroTour.svelte";
  import LogViewer from "$lib/components/LogViewer.svelte";
  import Troubleshooter from "$lib/components/Troubleshooter.svelte";
  import ResizeGrips from "$lib/components/ResizeGrips.svelte";
  import { CircleNotch, WarningCircle, ArrowsClockwise, Copy } from "phosphor-svelte";
  import { copyText } from "$lib/util";
  import { inTauri } from "$lib/ipc";

  onMount(() => {
    app.init();
    // WebKitGTK doesn't always fire DOM focus when the OS window is re-focused
    // (alt-tab back from Steam), so also listen to Tauri's own window event.
    // All three sources funnel into one throttled re-read.
    let unlisten: (() => void) | undefined;
    if (inTauri) {
      void import("@tauri-apps/api/window").then(({ getCurrentWindow }) =>
        getCurrentWindow()
          .onFocusChanged(({ payload: focused }) =>
            focused ? app.onWindowFocus() : app.onWindowBlur(),
          )
          .then((fn) => (unlisten = fn)),
      );
    }
    return () => unlisten?.();
  });
</script>

<svelte:window
  onkeydown={keys.handle}
  onfocus={() => app.onWindowFocus()}
  onblur={() => app.onWindowBlur()}
/>
<svelte:document onvisibilitychange={() => !document.hidden && app.onWindowFocus()} />

{#snippet loadErrorBanner()}
  {#if app.loadError}
    <div
      class="flex items-center gap-2 rounded-xl border px-4 py-2.5 text-xs text-red"
      style="border-color: color-mix(in srgb, var(--red) 35%, transparent); background: color-mix(in srgb, var(--red) 8%, transparent)"
    >
      <WarningCircle size={16} weight="fill" />
      {app.loadError}
    </div>
  {/if}
  {#if app.runtimeWarning}
    <!-- Yellow, not red: something's missing, not broken — matches the
         configWarnings/StaleBanner tone. Can never co-occur with loadError,
         since this is only ever set on the branch where Steam was found. -->
    <div
      class="flex items-start gap-2 rounded-xl border px-4 py-2.5 text-xs"
      style="border-color: color-mix(in srgb, var(--yellow) 35%, transparent); background: color-mix(in srgb, var(--yellow) 8%, transparent)"
    >
      <WarningCircle size={16} weight="fill" class="mt-0.5 shrink-0 text-yellow" />
      <span class="text-subtext">
        {app.runtimeWarning}
        <button
          class="ml-1 underline underline-offset-2 hover:text-text"
          onclick={() => (app.showSettings = true)}>Open Settings</button
        >
      </span>
    </div>
  {/if}
  <!-- Keyed on kind+path: a parse warning and a path warning can name the same
       file, and a duplicate key in a keyed {#each} is a crash. -->
  {#each app.configWarnings as w (w.kind + w.path)}
    <!-- Something the user configured was ignored. Without this the app just
         silently appears to disregard what they set. -->
    <div
      class="flex items-start gap-2 rounded-xl border px-4 py-2.5 text-xs"
      style="border-color: color-mix(in srgb, var(--yellow) 35%, transparent); background: color-mix(in srgb, var(--yellow) 8%, transparent)"
    >
      <WarningCircle size={16} weight="fill" class="mt-0.5 shrink-0 text-yellow" />
      <span class="text-subtext">
        {#if w.kind === "path"}
          protongen couldn't use the
          <code class="font-mono text-text">{w.file}</code> you set —
          <code class="font-mono text-text">{w.path}</code>
          (<span class="font-mono">{w.error}</span>).
          <button
            class="ml-1 underline underline-offset-2 hover:text-text"
            onclick={() => (app.showSettings = true)}>Open Settings</button
          >
        {:else if w.kind === "store"}
          Your settings file couldn't be read
          (<span class="font-mono">{w.error}</span>), so protongen started fresh.
          Your presets and per-game tuning weren't deleted — the old file was
          moved to <code class="font-mono text-text">{w.path}</code>.
        {:else}
          Your custom <code class="font-mono text-text">{w.file}</code> at
          <code class="font-mono text-text">{w.path}</code> couldn't be parsed
          (<span class="font-mono">{w.error}</span>); using the bundled
          {w.file === "recipes.toml" ? "recipes" : "catalog"}.
        {/if}
      </span>
    </div>
  {/each}
  {#if app.persistError}
    <!-- Sticky: settings are silently not being saved, which the user only
         discovers on exit. Cleared by the next successful write. -->
    <div
      class="flex items-start gap-2 rounded-xl border px-4 py-2.5 text-xs text-red"
      style="border-color: color-mix(in srgb, var(--red) 35%, transparent); background: color-mix(in srgb, var(--red) 8%, transparent)"
      role="alert"
    >
      <WarningCircle size={16} weight="fill" class="mt-0.5 shrink-0" />
      <span class="text-subtext">
        <span class="font-medium text-red">Your settings aren't being saved.</span>
        Changes will be lost when you quit. — {app.persistError}
      </span>
    </div>
  {/if}
{/snippet}

{#if app.initError}
  <div class="flex h-screen items-center justify-center p-8">
    <div class="flex max-w-lg flex-col items-start gap-3">
      <div class="flex items-center gap-2 text-red">
        <WarningCircle size={20} weight="fill" />
        <h1 class="text-base font-semibold">protongen couldn't start</h1>
      </div>
      <p class="text-sm text-subtext">
        Scanning your Steam install and Proton runtimes failed. This is usually
        temporary — retrying is safe.
      </p>
      <pre
        class="max-h-40 w-full overflow-auto rounded-xl border border-border bg-surface-2 p-3 font-mono text-xs text-subtext">{app.initError}</pre>
      <div class="flex items-center gap-2">
        <button
          onclick={() => app.init()}
          class="inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium transition active:scale-95"
          style="background: var(--accent); color: var(--on-accent)"
        >
          <ArrowsClockwise size={14} /> Retry
        </button>
        <button
          onclick={() => copyText(app.initError ?? "")}
          class="inline-flex items-center gap-1.5 rounded-lg bg-surface-2 px-3 py-1.5 text-sm text-subtext transition hover:text-text"
        >
          <Copy size={14} /> Copy details
        </button>
      </div>
    </div>
  </div>
{:else if !app.ready}
  <div class="flex h-screen items-center justify-center">
    <CircleNotch size={28} class="animate-spin text-accent" />
  </div>
{:else}
  <div class="flex h-screen flex-col">
    <Header />

    {#if app.view === "library"}
      <div class="min-h-0 flex-1 overflow-y-auto" in:fade={{ duration: 120 }}>
        {#if app.loadError || app.runtimeWarning || app.persistError || app.configWarnings.length || app.staleVisible || app.updateVisible}
          <div class="mx-auto flex w-full max-w-6xl flex-col gap-3 px-6 pt-4">
            {@render loadErrorBanner()}
            <UpdateBanner />
            <StaleBanner />
          </div>
        {/if}
        <Library />
      </div>
    {:else}
      <div class="flex min-h-0 flex-1" in:fade={{ duration: 120 }}>
        <!-- Advanced mode keeps the category rail; Simple mode drops it for a
             full-width curated card view. -->
        {#if app.uiMode === "advanced"}
          <NavRail />
        {/if}
        <main class="flex min-h-0 min-w-0 flex-1 flex-col">
          <!-- The single scrolling region: nothing else clips. -->
          <div class="min-h-0 flex-1 overflow-y-auto">
            <div class="mx-auto flex max-w-4xl flex-col gap-3 px-5 py-4">
              {@render loadErrorBanner()}
              <UpdateBanner />
              <StaleBanner />
              <Notices />
              {#if app.uiMode === "simple"}
                <SimplePanel />
              {:else}
                <MainPanel />
              {/if}
            </div>
          </div>

          <!-- Pinned command bar -->
          <div class="shrink-0 border-t border-border bg-mantle/30 px-5 py-3">
            <div class="mx-auto max-w-4xl">
              <CommandPreview />
            </div>
          </div>
        </main>
      </div>
    {/if}
  </div>
{/if}

<ShortcutsSheet />
<CommandPalette />
<!-- Mounted here, away from its two triggers in LauncherAction, so a view or
     section change can't unmount an open bits-ui modal — see HeroicConfirm. -->
<HeroicConfirm />
<!-- Same rationale, one level up: its trigger lives inside the MangoHud
     dialog, which the user can close mid-flow. -->
<MangoHudSystemConfirm />
<!-- Same rationale again, for vkBasalt's own system-wide export confirm. -->
<VkBasaltSystemConfirm />
<!-- Same rationale again: SimplePanel and MainPanel are mutually exclusive
     under the Simple/Advanced toggle, so a dialog living inside either one
     would unmount, open, the moment that toggle flips. -->
<OverlayBuilders />
<DefaultProfilePrompt />
<IntroTour />
<LogViewer />
<Troubleshooter />
<Toast />
<!-- Outside the init-error / loading branches above on purpose: both of those
     are full-screen and were unresizable too, which is exactly when you want to
     stretch the window to read a stack trace. -->
<ResizeGrips />
