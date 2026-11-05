<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { SquaresFour, Faders, ListChecks } from "phosphor-svelte";
  import type { UiMode } from "$lib/types";

  /**
   * The builder's scope: Essentials (the curated cards) or Everything (the full
   * catalog). It used to be a Simple / Advanced switch in the header that
   * swapped whole screens; it's now a scope over the same builder, and the On
   * column beside it never changes. Stored as `ui_mode` ("simple" =
   * Essentials), so existing state files carry over untouched.
   *
   * Also holds the On column's toggle for windows too narrow to dock it.
   */
  const modes: { mode: UiMode; label: string; icon: typeof SquaresFour }[] = [
    { mode: "simple", label: "Essentials", icon: SquaresFour },
    { mode: "advanced", label: "Everything", icon: Faders },
  ];
</script>

<div class="inline-flex items-center gap-1.5">
  <button
    onclick={() => (app.onColumnOpen = !app.onColumnOpen)}
    aria-pressed={app.onColumnOpen}
    title="What's on for this game"
    class="inline-flex h-8 items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 text-xs text-subtext transition hover:border-accent/50 min-[1100px]:hidden"
  >
    <ListChecks size={14} /> On
    <span
      class="rounded-full px-1 text-[10px]"
      style="background: color-mix(in srgb, var(--accent) 16%, transparent); color: var(--accent)"
      >{app.activeCount}</span
    >
  </button>
  <div class="inline-flex rounded-lg border border-border bg-surface-2/50 p-0.5" role="group" aria-label="Builder scope">
    {#each modes as m (m.mode)}
      <button
        onclick={() => {
          if (app.uiMode === m.mode) return;
          app.setUiMode(m.mode);
          // The other scope's offset means nothing here; start at its top.
          document.querySelector("[data-scroll-root]")?.scrollTo({ top: 0 });
        }}
        aria-pressed={app.uiMode === m.mode}
        class="inline-flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs transition {app.uiMode === m.mode
          ? 'font-medium'
          : 'text-muted hover:text-text'}"
        style={app.uiMode === m.mode ? "background: var(--accent); color: var(--on-accent)" : ""}
      >
        <m.icon size={13} weight="fill" />
        {m.label}
      </button>
    {/each}
  </div>
</div>
