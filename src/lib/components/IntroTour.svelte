<script lang="ts">
  import { app } from "$lib/state.svelte";
  import Dialog from "./Dialog.svelte";
  import { GameController, Sparkle, ClipboardText, ArrowRight } from "phosphor-svelte";

  /**
   * A one-time, four-step walkthrough of Simple mode, shown the first time a
   * user lands there. Mounted once at the app root (App.svelte), like
   * HeroicConfirm/DefaultProfilePrompt.
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
      body: "Simple mode shows the options people reach for most — upscaling, frame pacing, HDR — as plain switches. Advanced mode (top-right) exposes the full catalog if you ever need it.",
    },
    {
      icon: ClipboardText,
      title: "Copy the command",
      body: "The bar at the bottom always shows the exact Steam launch command or umu-run command for your current selections. Copy it into the game's launch options and you're done.",
    },
  ];

  let step = $state(0);
  const last = $derived(step === steps.length - 1);

  function next() {
    if (last) open = false;
    else step += 1;
  }
</script>

<Dialog bind:open title="Welcome to protongen" width="26rem">
  <div class="space-y-4">
    {#each [steps[step]] as s}
      <div class="flex items-start gap-3">
        <div class="grid size-9 shrink-0 place-items-center rounded-lg bg-accent/10 text-accent">
          <s.icon size={18} weight="duotone" />
        </div>
        <div class="min-w-0">
          <h3 class="text-sm font-medium text-text">{s.title}</h3>
          <p class="mt-1 text-sm text-muted">{s.body}</p>
        </div>
      </div>
    {/each}

    <div class="flex items-center justify-between pt-1">
      <div class="flex gap-1">
        {#each steps as _, i}
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
