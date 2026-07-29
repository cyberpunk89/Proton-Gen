<script lang="ts">
  import Popover from "./Popover.svelte";
  import { openUrl } from "$lib/util";
  import { Info, ArrowSquareOut } from "phosphor-svelte";

  let {
    details = null,
    example = null,
    url = null,
    defaultValue = "",
    values = [],
  }: {
    details?: string | null;
    example?: string | null;
    url?: string | null;
    defaultValue?: string;
    values?: string[];
  } = $props();
</script>

<!-- trapFocus={false}: a read-only bubble shouldn't capture the keyboard. -->
<Popover width="22rem" trapFocus={false}>
  {#snippet trigger({ props, open })}
    <button
      {...props}
      type="button"
      class="grid size-6 place-items-center rounded-md text-muted transition hover:bg-surface-2 hover:text-blue {open
        ? 'bg-surface-2 text-blue'
        : ''}"
      aria-label="More info"
    >
      <Info size={16} weight="bold" />
    </button>
  {/snippet}

  <div class="space-y-2 text-sm">
    {#if details}
      <p class="leading-relaxed text-subtext">{details}</p>
    {/if}
    {#if defaultValue}
      <p class="text-xs text-muted">
        Default: <code class="font-mono text-text">{defaultValue}</code>
      </p>
    {/if}
    {#if values.length}
      <p class="text-xs text-muted">
        Values:
        {#each values as v, i (v)}<code class="font-mono text-text"
            >{v}</code
          >{#if i < values.length - 1}, {/if}{/each}
      </p>
    {/if}
    {#if example}
      <pre
        class="overflow-x-auto rounded-lg bg-mantle p-2 font-mono text-xs text-subtext">{example}</pre>
    {/if}
    {#if url}
      <button
        type="button"
        onclick={() => openUrl(url)}
        class="inline-flex items-center gap-1 text-xs text-blue hover:underline"
      >
        <ArrowSquareOut size={13} weight="bold" /> Documentation
      </button>
    {/if}
  </div>
</Popover>
