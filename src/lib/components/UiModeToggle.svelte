<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { SquaresFour, Faders } from "phosphor-svelte";
  import type { UiMode } from "$lib/types";

  // Mirrors ModeToggle.svelte (Steam/umu) — same segmented-control markup, so the
  // two density switches read as siblings.
  const modes: { mode: UiMode; label: string; icon: typeof SquaresFour }[] = [
    { mode: "simple", label: "Simple", icon: SquaresFour },
    { mode: "advanced", label: "Advanced", icon: Faders },
  ];
</script>

<div class="inline-flex rounded-xl border border-border bg-surface-2/60 p-1">
  {#each modes as m (m.mode)}
    <button
      onclick={() => app.setUiMode(m.mode)}
      class="inline-flex items-center gap-1.5 rounded-lg px-2.5 py-1 text-xs font-medium transition"
      style={app.uiMode === m.mode
        ? "background: var(--accent); color: var(--on-accent)"
        : "color: var(--muted)"}
    >
      <m.icon size={14} weight="fill" />
      {m.label}
    </button>
  {/each}
</div>
