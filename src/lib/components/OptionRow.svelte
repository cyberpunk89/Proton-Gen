<script lang="ts">
  import Switch from "./Switch.svelte";
  import Badges from "./Badges.svelte";
  import InfoPopover from "./InfoPopover.svelte";

  let {
    enabled = false,
    title,
    mono = false,
    help = "",
    details = null,
    example = null,
    url = null,
    defaultValue = "",
    values = [],
    valueField = "none",
    value = "",
    placeholder = "",
    requires = null,
    gpu = null,
    needs = [],
    dim = false,
    onToggle,
    onValue,
  }: {
    enabled?: boolean;
    title: string;
    mono?: boolean;
    help?: string;
    details?: string | null;
    example?: string | null;
    url?: string | null;
    defaultValue?: string;
    values?: string[];
    valueField?: "none" | "text" | "select" | "segmented";
    value?: string;
    placeholder?: string;
    requires?: string | null;
    gpu?: string | null;
    needs?: string[];
    dim?: boolean;
    onToggle: () => void;
    onValue?: (v: string) => void;
  } = $props();
</script>

<div
  class="rounded-xl px-3 py-2 transition-colors {enabled ? '' : 'hover:bg-surface-2/40'}"
  style="{enabled
    ? 'background: color-mix(in srgb, var(--accent) 9%, transparent);'
    : ''}{dim ? 'opacity:.55' : ''}"
>
  <div class="flex items-center gap-3">
    <Switch checked={enabled} onchange={onToggle} label={title} />

    <span
      class="min-w-0 flex-1 truncate {mono ? 'font-mono text-[13px]' : 'text-sm'}"
      style="color: {enabled ? 'var(--text)' : 'var(--subtext)'}; font-weight: {enabled
        ? 500
        : 400}"
    >
      {title}
    </span>

    {#if valueField === "text"}
      <input
        class="w-40 rounded-lg border border-border bg-surface-2 px-2 py-1 font-mono text-xs text-text outline-none focus:border-accent"
        {placeholder}
        {value}
        oninput={(e) => onValue?.(e.currentTarget.value)}
      />
    {:else if valueField === "select"}
      <select
        class="w-40 rounded-lg border border-border bg-surface-2 px-2 py-1 text-xs text-text outline-none focus:border-accent"
        {value}
        onchange={(e) => onValue?.(e.currentTarget.value)}
      >
        {#each values as v (v)}
          <option value={v}>{v}</option>
        {/each}
      </select>
    {:else if valueField === "segmented"}
      <div class="inline-flex shrink-0 overflow-hidden rounded-lg border border-border">
        {#each values as v, i (v)}
          <button
            type="button"
            onclick={() => onValue?.(v)}
            class="px-3 py-1 font-mono text-xs transition focus-visible:outline focus-visible:-outline-offset-2 focus-visible:outline-2 focus-visible:outline-accent {i > 0
              ? 'border-l border-border'
              : ''} {value === v ? 'font-medium' : 'text-muted hover:text-subtext'}"
            style={value === v
              ? "background: color-mix(in srgb, var(--accent) 22%, transparent); color: var(--accent)"
              : ""}
          >
            {v}
          </button>
        {/each}
      </div>
    {/if}

    <div class="flex items-center gap-1.5">
      <Badges {requires} {gpu} {needs} />
      <InfoPopover {details} {example} {url} {defaultValue} {values} />
    </div>
  </div>

  {#if help}
    <p class="mt-1 pl-[50px] text-xs leading-snug text-muted">{help}</p>
  {/if}
</div>
