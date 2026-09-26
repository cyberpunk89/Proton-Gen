<script lang="ts">
  import { app } from "$lib/state.svelte";
  import Dialog from "./Dialog.svelte";
  import Badges from "./Badges.svelte";
  import {
    GameController,
    Sparkle,
    ClipboardText,
    ArrowRight,
    Desktop,
    CheckCircle,
    XCircle,
  } from "phosphor-svelte";

  /**
   * A one-time walkthrough of Simple mode, shown the first time a user lands
   * there. Mounted once at the app root (App.svelte), like
   * HeroicConfirm/DefaultProfilePrompt.
   *
   * Step 0 ("Your system") is dynamic, not static copy: it reads live
   * discovery results (steamRoot/loadError, runtimeWarning, requiresStatus)
   * so a first-time user on an unfamiliar distro or layout sees exactly what
   * protongen found before the feature tour starts, with a direct link into
   * Settings → Paths. Steps 1-3 are the original static feature tour.
   *
   * Deliberately just a stepped dialog rather than a real DOM spotlight over
   * the library/cards/command bar: those move and resize under real content,
   * and a wrong-anchored highlight would be worse than none.
   *
   * `open` starts `false` and is armed by the `$effect` below only once
   * `app.ready` — never bound directly to a value computed during `load()`.
   * bits-ui's Dialog needs a real closed→open transition to register its body
   * scroll-lock correctly; a dialog that renders already-open on its very
   * first paint (as this one did before) leaves `body { pointer-events: none
   * }` stuck forever after it closes, click-killing the whole app the same
   * way #63 did.
   */
  let open = $state(false);
  $effect(() => {
    if (app.ready && app.uiMode === "simple" && !app.store.seen_intro_tour) open = true;
  });

  // "Replay intro tour" in Settings → About (which has already closed itself).
  $effect(() => {
    if (app.tourReplay === 0) return;
    step = 0;
    open = true;
  });

  // Mark seen on any close — Skip, Get started, or the dialog's own X/Escape —
  // without re-firing on mount (`open` starts `false` too, but that's not a
  // close, it's "never shown yet").
  let wasOpen = false;
  $effect(() => {
    if (open) wasOpen = true;
    else if (wasOpen) {
      wasOpen = false;
      app.markTourSeen();
    }
  });

  const steps = [
    {
      icon: GameController,
      title: "Pick a game",
      body: "Start from the library — every installed Steam game and non-Steam shortcut shows up there. Click one to open its builder.",
    },
    {
      icon: Sparkle,
      title: "Toggle what you need",
      body: "Simple mode shows the options people reach for most — upscaling, frame pacing, HDR — as plain switches. Once a game is open, the Simple / Advanced switch in the header exposes the full catalog if you ever need it.",
    },
    {
      icon: ClipboardText,
      title: "Copy the command",
      body: "The bar at the bottom always shows the exact Steam launch command or umu-run command for your current selections. Copy it, paste it into the game's launch options in Steam, and switch back — protongen checks and marks it applied.",
    },
  ];

  // Step 0 is the dynamic "Your system" summary below; the static feature
  // steps above start at index 1.
  const totalSteps = steps.length + 1;
  const dotIndices = Array.from({ length: totalSteps }, (_, i) => i);

  let step = $state(0);
  const last = $derived(step === totalSteps - 1);
  const current = $derived(step === 0 ? null : steps[step - 1]);

  function next() {
    if (last) open = false;
    else step += 1;
  }

  function openSettings() {
    open = false;
    // After the dialog's close finishes, so two modal layers never overlap (#63).
    setTimeout(() => app.openSettings("paths"), 250);
  }
</script>

<Dialog bind:open title="Welcome to protongen" width="28rem">
  <div class="space-y-4">
    {#if step === 0}
      <div class="space-y-3">
        <div class="flex items-start gap-3">
          <div class="grid size-9 shrink-0 place-items-center rounded-lg bg-accent/10 text-accent">
            <Desktop size={18} weight="duotone" />
          </div>
          <div class="min-w-0">
            <h3 class="text-sm font-medium text-text">Your system</h3>
            <p class="mt-1 text-sm text-muted">
              What protongen found here, before the tour — every distro and layout looks a
              little different.
            </p>
          </div>
        </div>

        <div class="ml-12 space-y-1.5">
          <div class="flex items-start gap-1.5 text-xs">
            {#if app.steamRoot}
              <CheckCircle size={14} weight="fill" class="mt-0.5 shrink-0 text-green" />
              <span class="text-subtext">
                Steam found at <code class="font-mono text-text">{app.steamRoot}</code>
              </span>
            {:else}
              <XCircle size={14} weight="fill" class="mt-0.5 shrink-0 text-red" />
              <span class="text-subtext">{app.loadError ?? "No Steam install found."}</span>
            {/if}
          </div>

          {#if app.steamRoot}
            <div class="flex items-start gap-1.5 text-xs">
              {#if app.runtimeWarning}
                <XCircle size={14} weight="fill" class="mt-0.5 shrink-0 text-red" />
                <span class="text-subtext">{app.runtimeWarning}</span>
              {:else}
                <CheckCircle size={14} weight="fill" class="mt-0.5 shrink-0 text-green" />
                <span class="text-subtext">
                  {app.runtimes.filter((r) => r.kind !== "auto").length} Proton runtime(s) found
                </span>
              {/if}
            </div>
          {/if}

          {#if Object.keys(app.requiresStatus).length}
            <div class="flex flex-wrap items-center gap-1.5 pt-0.5">
              {#each Object.keys(app.requiresStatus).sort() as name (name)}
                <Badges requires={name} />
              {/each}
            </div>
          {/if}
        </div>

        <button
          onclick={openSettings}
          class="ml-12 text-xs text-accent underline underline-offset-2 hover:opacity-80"
        >
          Open Settings → Paths
        </button>
      </div>
    {:else if current}
      <div class="flex items-start gap-3">
        <div class="grid size-9 shrink-0 place-items-center rounded-lg bg-accent/10 text-accent">
          <current.icon size={18} weight="duotone" />
        </div>
        <div class="min-w-0">
          <h3 class="text-sm font-medium text-text">{current.title}</h3>
          <p class="mt-1 text-sm text-muted">{current.body}</p>
        </div>
      </div>
    {/if}

    <div class="flex items-center justify-between pt-1">
      <div class="flex gap-1">
        {#each dotIndices as i (i)}
          <span
            class="size-1.5 rounded-full transition"
            class:bg-accent={i === step}
            class:bg-border={i !== step}
          ></span>
        {/each}
      </div>
      <div class="flex items-center gap-2">
        <button onclick={() => (open = false)} class="px-2 py-1.5 text-xs text-muted hover:text-text">
          Skip
        </button>
        <button
          onclick={next}
          class="inline-flex items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-on-accent transition hover:opacity-90"
        >
          {last ? "Get started" : "Next"}
          {#if !last}<ArrowRight size={13} />{/if}
        </button>
      </div>
    </div>
  </div>
</Dialog>
