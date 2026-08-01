<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import { slide } from "$lib/motion.svelte";
  import { WarningOctagon, Warning, Info, Wrench, ArrowRight } from "phosphor-svelte";
  import type { Notice, Severity } from "$lib/types";

  let open = $state(true);

  const LOOK: Record<Severity, { colour: string; icon: typeof Warning; label: string }> = {
    error: { colour: "var(--red)", icon: WarningOctagon, label: "error" },
    warning: { colour: "var(--peach)", icon: Warning, label: "warning" },
    info: { colour: "var(--blue)", icon: Info, label: "hint" },
  };

  const RANK: Record<Severity, number> = { error: 0, warning: 1, info: 2 };

  let sorted = $derived([...app.notices].sort((a, b) => RANK[a.severity] - RANK[b.severity]));
  /** The card takes its colour and count from the worst thing in it, so an error
   *  can't hide behind a pile of hints the way one flat peach card let it. */
  let worst = $derived<Severity>(sorted[0]?.severity ?? "info");
  let worstLook = $derived(LOOK[worst]);
  let worstCount = $derived(sorted.filter((n) => n.severity === worst).length);

  function jump(key: string) {
    if (!app.revealParam(key)) toast.info(`${key} isn't in the catalog.`);
  }

  /**
   * Apply a notice's remedy through the ordinary mutators, which is what makes it
   * undoable for free — they all funnel into the history stack.
   */
  function applyFix(n: Notice) {
    const fix = n.fix;
    if (!fix) return;
    for (const key of fix.disable) {
      if (app.env[key]?.enabled) app.toggleEnv(key);
      if (app.wrap[key]?.enabled) app.toggleWrap(key);
    }
    for (const [key, value] of fix.enable) {
      app.setEnvValue(key, value);
      if (!app.env[key]?.enabled) app.toggleEnv(key);
    }
    toast.success(fix.label, { action: { label: "Undo", onClick: () => app.undo() } });
  }
</script>

{#if sorted.length}
  {@const HeaderIcon = worstLook.icon}
  <div
    class="rounded-xl border p-3"
    style="border-color: color-mix(in srgb, {worstLook.colour} 35%, transparent);
           background: color-mix(in srgb, {worstLook.colour} 8%, transparent)"
    transition:slide={{ duration: 150 }}
  >
    <button
      class="flex w-full items-center gap-2"
      onclick={() => (open = !open)}
      aria-expanded={open}
      aria-controls="notices-list"
    >
      <HeaderIcon size={15} weight="fill" style="color: {worstLook.colour}" />
      <!-- Counts the worst severity, not the bare total: "3 notices" reads the
           same whether they are three hints or three errors. -->
      <span class="text-xs font-medium" style="color: {worstLook.colour}">
        {worstCount}
        {worstLook.label}{worstCount > 1 ? "s" : ""}{sorted.length > worstCount
          ? ` · ${sorted.length - worstCount} more`
          : ""}
      </span>
      <span class="ml-auto text-[11px] text-muted">{open ? "hide" : "show"}</span>
    </button>

    {#if open}
      <ul id="notices-list" class="mt-2 space-y-2" transition:slide={{ duration: 120 }}>
        {#each sorted as n (n.id)}
          {@const look = LOOK[n.severity]}
          {@const Icon = look.icon}
          <li class="flex items-start gap-2 text-xs">
            <Icon size={13} weight="fill" class="mt-0.5 shrink-0" style="color: {look.colour}" />
            <div class="min-w-0 flex-1">
              <p class="text-subtext">{n.message}</p>
              <div class="mt-1 flex flex-wrap items-center gap-1.5">
                {#each n.keys as key (key)}
                  <!-- revealParam carries the hardware-relevance guard, so these
                       work even for rows the panel currently filters out — which
                       is exactly the case for nvapi-without-nvidia. -->
                  <button
                    onclick={() => jump(key)}
                    title="Go to {key}"
                    class="inline-flex items-center gap-0.5 rounded-md bg-surface-2/70 px-1.5 py-0.5 font-mono text-[10px] text-subtext transition hover:text-accent"
                  >
                    {key}<ArrowRight size={9} weight="bold" />
                  </button>
                {/each}
                {#if n.fix}
                  <button
                    onclick={() => applyFix(n)}
                    class="inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-medium transition hover:brightness-110"
                    style="background: color-mix(in srgb, {look.colour} 18%, transparent); color: {look.colour}"
                  >
                    <Wrench size={10} weight="bold" />
                    {n.fix.label}
                  </button>
                {/if}
              </div>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{/if}
