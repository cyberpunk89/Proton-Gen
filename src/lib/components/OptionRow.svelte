<script lang="ts">
  import Switch from "./Switch.svelte";
  import Badges from "./Badges.svelte";
  import InfoPopover from "./InfoPopover.svelte";
  import { app } from "$lib/state.svelte";
  import { prefersReducedMotion } from "$lib/motion.svelte";

  let {
    /** Catalog key. Distinct from `title`, which for wrappers is the label. */
    paramKey = "",
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
    titleRanges = [],
    helpRanges = [],
    onToggle,
    onValue,
  }: {
    paramKey?: string;
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
    /** Half-open [start, end) spans to highlight, from the fuzzy matcher. */
    titleRanges?: [number, number][];
    helpRanges?: [number, number][];
    onToggle: () => void;
    onValue?: (v: string) => void;
  } = $props();

  /**
   * Slice a string into plain/highlighted alternating parts.
   *
   * Deliberately not `{@html}` with <mark> injected: params.toml is overridable
   * from $XDG_CONFIG_HOME, so help text is untrusted-ish input and must never be
   * parsed as markup.
   */
  function segments(text: string, ranges: [number, number][]) {
    if (!ranges.length) return [{ text, hit: false }];
    const out: { text: string; hit: boolean }[] = [];
    let at = 0;
    for (const [start, end] of ranges) {
      if (start >= text.length) break;
      const s = Math.max(at, start);
      const e = Math.min(text.length, end);
      if (s > at) out.push({ text: text.slice(at, s), hit: false });
      if (e > s) out.push({ text: text.slice(s, e), hit: true });
      at = Math.max(at, e);
    }
    if (at < text.length) out.push({ text: text.slice(at), hit: false });
    return out;
  }

  // The visible title is the switch's accessible name, so it needs an id.
  const uid = $props.id();
  const labelId = `opt-title-${uid}`;

  let row = $state<HTMLDivElement | null>(null);
  let flash = $state(false);
  let flashTimer: ReturnType<typeof setTimeout> | null = null;

  /**
   * Respond to `app.revealParam`. Reads the nonce so a repeat jump to the same
   * key re-fires; without it the second click on a lint notice would do nothing.
   *
   * After `setSection` the target row mounts in the same tick, so this fires
   * correctly even when the jump crossed a category.
   */
  $effect(() => {
    const target = app.focusParam;
    if (!target || !paramKey || target.key !== paramKey || !row) return;
    void target.nonce;

    row.scrollIntoView({
      block: "center",
      behavior: prefersReducedMotion() ? "auto" : "smooth",
    });
    // Focus the switch rather than the row: it is the thing you came here to
    // operate, and it is already the row's labelled control.
    row.querySelector<HTMLElement>('[role="switch"]')?.focus();

    flash = true;
    if (flashTimer) clearTimeout(flashTimer);
    flashTimer = setTimeout(() => (flash = false), 1200);
  });
</script>

<div
  bind:this={row}
  id={paramKey ? `param-${paramKey}` : undefined}
  class="rounded-xl px-3 py-2 transition-colors {enabled
    ? ''
    : 'hover:bg-surface-2/40'} {flash ? 'ring-2 ring-accent' : ''}"
  style="{enabled
    ? 'background: color-mix(in srgb, var(--accent) 9%, transparent);'
    : ''}{dim ? 'opacity:.55' : ''}"
>
  <div class="flex items-center gap-3">
    <Switch checked={enabled} onchange={onToggle} labelledby={labelId} />

    <span
      id={labelId}
      class="min-w-0 flex-1 truncate {mono ? 'font-mono text-[13px]' : 'text-sm'}"
      style="color: {enabled ? 'var(--text)' : 'var(--subtext)'}; font-weight: {enabled
        ? 500
        : 400}"
    >
      {#each segments(title, titleRanges) as seg, i (i)}{#if seg.hit}<mark
            class="rounded-[3px] bg-transparent text-inherit"
            style="background: color-mix(in srgb, var(--accent) 25%, transparent)"
            >{seg.text}</mark
          >{:else}{seg.text}{/if}{/each}
    </span>

    {#if valueField === "text"}
      <input
        class="w-40 rounded-lg border border-border bg-surface-2 px-2 py-1 font-mono text-xs text-text outline-none focus:border-accent"
        aria-label="{title} value"
        {placeholder}
        {value}
        oninput={(e) => onValue?.(e.currentTarget.value)}
      />
    {:else if valueField === "select"}
      <select
        class="w-40 rounded-lg border border-border bg-surface-2 px-2 py-1 text-xs text-text outline-none focus:border-accent"
        aria-label="{title} value"
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
          <!-- Inset focus offset only: the group clips with overflow-hidden, so
               the global outward ring from app.css would be cut off. -->
          <button
            type="button"
            onclick={() => onValue?.(v)}
            class="px-3 py-1 font-mono text-xs transition focus-visible:-outline-offset-2 {i > 0
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
    <p class="mt-1 pl-[50px] text-xs leading-snug text-muted">
      {#each segments(help, helpRanges) as seg, i (i)}{#if seg.hit}<mark
            class="rounded-[3px] bg-transparent text-inherit"
            style="background: color-mix(in srgb, var(--accent) 25%, transparent)"
            >{seg.text}</mark
          >{:else}{seg.text}{/if}{/each}
    </p>
  {/if}
</div>
