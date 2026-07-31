<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import Popover from "./Popover.svelte";
  import OpenInSteam from "./OpenInSteam.svelte";
  import { CheckCircle, WarningCircle, CircleDashed, DownloadSimple } from "phosphor-svelte";

  /**
   * "Is what I built actually live in Steam?" — answered where the user is
   * already looking. Renders nothing at all unless the question makes sense; see
   * `app.syncState`, which folds away no-game, non-Steam shortcuts and umu mode.
   *
   * Used twice: the command bar and GameRuntimePanel (#40).
   */

  const look = {
    "in-sync": { icon: CheckCircle, colour: "var(--green)" },
    drifted: { icon: WarningCircle, colour: "var(--peach)" },
    "not-applied": { icon: CircleDashed, colour: "var(--muted)" },
  } as const;

  let state = $derived(app.syncState);
  let d = $derived(app.launchDiff);
  let mismatch = $derived(app.runtimeMismatch);

  let label = $derived.by(() => {
    if (state === "in-sync") return "In sync with Steam";
    if (state === "not-applied") return "Not in Steam yet";
    const n = app.driftCount;
    return `${n} ${n === 1 ? "change" : "changes"} not pasted`;
  });

  /** Compact form for a tight toolbar — the full wording costs ~148px, which is
   *  most of what the command bar has to spare. The glyph already carries the
   *  severity, the popover carries the detail, and `title`/`aria-label` keep the
   *  full sentence for hover and assistive tech. */
  let shortLabel = $derived.by(() => {
    if (state === "in-sync") return "Synced";
    if (state === "not-applied") return "Not set";
    const n = app.driftCount;
    // Keeps the noun: a bare "6" next to a warning glyph reads as a mystery.
    return `${n} ${n === 1 ? "change" : "changes"}`;
  });

  async function loadCurrent() {
    const current = app.currentLaunchOptions;
    if (!current) return;
    await app.importCommand(current);
    toast.success("Loaded current launch options");
  }
</script>

{#if state !== "hidden"}
  {@const l = look[state]}
  <Popover align="end" width="24rem">
    {#snippet trigger({ props, open })}
      <button
        {...props}
        type="button"
        title={label}
        aria-label={label}
        class="inline-flex min-w-0 shrink items-center gap-1.5 rounded-full border px-2 py-1 text-[11px] font-medium transition hover:brightness-110 {open
          ? 'brightness-110'
          : ''}"
        style="border-color: color-mix(in srgb, {l.colour} 35%, transparent);
               background: color-mix(in srgb, {l.colour} 12%, transparent);
               color: {l.colour}"
      >
        <l.icon size={13} weight="fill" class="shrink-0" />
        <span class="truncate @xl:hidden">{shortLabel}</span>
        <span class="hidden truncate @xl:inline">{label}</span>
      </button>
    {/snippet}

    <div class="space-y-3 text-sm">
      <p class="text-xs font-medium text-text">{label}</p>

      {#if state === "not-applied"}
        <p class="text-xs leading-relaxed text-subtext">
          Steam has no launch options set for this game. Copy the command and paste
          it into the Launch Options box.
        </p>
      {/if}

      {#if d && state === "drifted"}
        <!-- Short mono lines: the point is to be scannable, not exhaustive. -->
        <ul class="space-y-1 font-mono text-[11px] leading-relaxed">
          {#each d.added as k (k)}
            <li class="text-green">+ {k}</li>
          {/each}
          {#each d.removed as k (k)}
            <li class="text-red">− {k}</li>
          {/each}
          {#each d.changed as c (c.key)}
            <li class="text-peach">
              ~ {c.key}: <span class="text-muted">{c.current}</span> → {c.built}
            </li>
          {/each}
          {#if d.game_args}
            <li class="text-peach">
              ~ game args: <span class="text-muted">{d.game_args.current || "(none)"}</span>
              → {d.game_args.built || "(none)"}
            </li>
          {/if}
          {#each d.unmodeled as u (u)}
            <!-- protongen can't represent these, so it can't promise anything
                 about them beyond "they're there". -->
            <li class="text-muted">? {u} <span class="not-italic">(not modelled)</span></li>
          {/each}
        </ul>
      {/if}

      {#if mismatch}
        <!-- Deliberately a separate line, not folded into the drift verdict: the
             Proton dropdown is its own Steam control with its own paste target. -->
        <p
          class="flex items-start gap-1.5 border-t border-border/50 pt-2 text-xs text-subtext"
        >
          <DownloadSimple size={13} class="mt-0.5 shrink-0 text-peach" />
          <span>
            {#if mismatch.steam}
              Steam's Proton is set to
              <code class="font-mono text-text">{mismatch.steam}</code> — change it to
              <code class="font-mono text-text">{mismatch.wanted}</code>.
            {:else}
              Also set Steam's Proton dropdown to
              <code class="font-mono text-text">{mismatch.wanted}</code>.
            {/if}
          </span>
        </p>
      {/if}

      <div class="flex flex-wrap items-center gap-1.5 border-t border-border/50 pt-2">
        <OpenInSteam />
        {#if app.currentLaunchOptions}
          <button
            type="button"
            onclick={loadCurrent}
            class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50 active:scale-95"
          >
            Load current
          </button>
        {/if}
      </div>
    </div>
  </Popover>
{/if}
