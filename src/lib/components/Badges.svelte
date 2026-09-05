<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { irrelevance, copyText } from "$lib/util";
  import { toast } from "$lib/toast.svelte";
  import { CheckCircle, XCircle, Copy } from "phosphor-svelte";

  let {
    requires = null,
    pkg = null,
    gpu = null,
    needs = [],
  }: {
    requires?: string | null;
    pkg?: string | null;
    gpu?: string | null;
    needs?: string[];
  } = $props();

  let installed = $derived(
    requires ? (app.requiresStatus[requires] ?? false) : null,
  );
  let irrelevant = $derived(irrelevance(app.hwCaps, gpu, needs));

  async function copyInstallCommand() {
    await copyText(`sudo pacman -S ${pkg}`);
    toast.success(`Copied: sudo pacman -S ${pkg}`);
  }
</script>

{#if irrelevant}
  <span
    class="rounded-full bg-surface-2 px-2 py-0.5 text-[11px] text-muted"
    title="This option won't apply on your machine"
  >
    {irrelevant}
  </span>
{/if}

{#if requires}
  {#if installed}
    <span
      class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px]"
      style="background: color-mix(in srgb, var(--green) 18%, transparent); color: var(--green)"
      title="{requires} found on $PATH"
    >
      <CheckCircle size={12} weight="fill" />{requires}
    </span>
  {:else}
    <span
      class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px]"
      style="background: color-mix(in srgb, var(--red) 18%, transparent); color: var(--red)"
      title="{requires} not found on $PATH"
    >
      <XCircle size={12} weight="fill" />{requires}
    </span>
    {#if pkg}
      <button
        type="button"
        onclick={copyInstallCommand}
        class="inline-flex items-center gap-1 rounded-full border border-border px-2 py-0.5 text-[11px] text-subtext transition hover:border-accent/50 hover:text-text"
        title="Copy: sudo pacman -S {pkg}"
      >
        <Copy size={12} />Copy install command
      </button>
    {/if}
  {/if}
{/if}
