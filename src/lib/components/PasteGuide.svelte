<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { CheckCircle, Circle, X } from "phosphor-svelte";
  import { fade } from "$lib/motion.svelte";

  /**
   * The "now paste it" checklist that Copy & open Steam leaves under the
   * command. Each step ticks itself off from data the app already re-reads on
   * window focus (the sync verdict and Steam's compat-tool mapping), so coming
   * back from Steam shows what landed and what is still missing — instead of a
   * one-line hint that reads the same before and after.
   *
   * The runtime step is only checkable for runtimes with a real internal name;
   * valve/auto ones stay an unticked instruction, same as the plain hint.
   */
  let pasted = $derived(app.syncState === "in-sync");
  let runtimeStep = $derived.by(() => {
    const r = app.selectedRuntime;
    if (!r) return null;
    if (!app.runtimeComparable) {
      return { done: false, text: `Compatibility tab: set Proton to ${r.display_name}` };
    }
    const m = app.runtimeMismatch;
    return {
      done: m === null,
      text: m === null
        ? `Compatibility tab: Proton is ${r.display_name}`
        : `Compatibility tab: set Proton to ${r.display_name}`,
    };
  });
  let allDone = $derived(pasted && (runtimeStep?.done ?? true));

  // Once everything checks out, give the ticks a moment to be seen, then fold.
  $effect(() => {
    if (!allDone) return;
    const t = setTimeout(() => (app.pasteGuideFor = null), 4000);
    return () => clearTimeout(t);
  });

  const steps = $derived([
    { done: true, text: "Command copied" },
    { done: pasted, text: "General tab: paste into Launch Options" },
    ...(runtimeStep ? [runtimeStep] : []),
  ]);
</script>

<div
  transition:fade={{ duration: 150 }}
  class="mt-3 rounded-lg border border-border/60 bg-surface-2/40 px-3 py-2 text-xs"
  role="status"
>
  <div class="mb-1 flex items-center gap-2">
    <span class="font-medium text-text">
      {allDone ? "All set in Steam" : "Finish in Steam's Properties window"}
    </span>
    <button
      onclick={() => (app.pasteGuideFor = null)}
      class="ml-auto grid size-5 place-items-center rounded text-muted transition hover:text-text"
      aria-label="Hide checklist"
      title="Hide checklist"
    >
      <X size={12} />
    </button>
  </div>
  <ol class="space-y-0.5">
    {#each steps as step (step.text)}
      <li class="flex items-center gap-1.5" class:text-green={step.done} class:text-subtext={!step.done}>
        {#if step.done}
          <CheckCircle size={13} weight="fill" class="shrink-0" />
        {:else}
          <Circle size={13} class="shrink-0" />
        {/if}
        <span>{step.text}</span>
      </li>
    {/each}
  </ol>
  {#if !allDone}
    <p class="mt-1 text-muted">Ticks update when you switch back to this window.</p>
  {/if}
</div>
