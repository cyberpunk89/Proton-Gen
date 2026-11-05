<script lang="ts">
  import Switch from "./Switch.svelte";
  import SelectField from "./SelectField.svelte";
  import Badges from "./Badges.svelte";
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import { prefersReducedMotion, slide } from "$lib/motion.svelte";
  import { openUrl } from "$lib/util";
  import { shQuote } from "$lib/shell";
  import { Sparkle, CaretDown, ArrowSquareOut, Wrench } from "phosphor-svelte";
  import type { Snippet } from "svelte";
  import type { Notice, Severity } from "$lib/types";

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
    pkg = null,
    gpu = null,
    needs = [],
    dim = false,
    appliedBy = null,
    titleRanges = [],
    helpRanges = [],
    action = null,
    notices = [],
    wrapper = false,
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
    pkg?: string | null;
    gpu?: string | null;
    needs?: string[];
    dim?: boolean;
    /** Name of the recipe that set this row, if any. */
    appliedBy?: string | null;
    /** Half-open [start, end) spans to highlight, from the fuzzy matcher. */
    titleRanges?: [number, number][];
    helpRanges?: [number, number][];
    /**
     * An extra control for rows that own more than a switch and a value — a
     * link into a dedicated editor, say. Rendered in the trailing cluster,
     * before the badges. Generic on purpose: the alternative was a
     * `mangohud`-shaped special case baked into MainPanel's row snippet.
     */
    action?: Snippet | null;
    /** Lint notices about this row, shown on it with their fix. */
    notices?: Notice[];
    /** A wrapper program, so "Emits" shows `key args`, not `KEY=value`. */
    wrapper?: boolean;
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

  /** Details unfold in place — no popover layer (fewer #63-style risks), and
   *  the row's own context stays on screen while you read. */
  let expanded = $state(false);

  /** What this row puts on the command line when on. */
  let emits = $derived(
    wrapper
      ? [paramKey, value].filter(Boolean).join(" ")
      : `${paramKey}=${shQuote(value || defaultValue || "1")}`,
  );

  const SEV: Record<Severity, string> = {
    error: "var(--red)",
    warning: "var(--peach)",
    info: "var(--blue)",
  };

  /** Apply a notice's remedy as one history entry (same as Notices.svelte). */
  function applyFix(n: Notice) {
    const fix = n.fix;
    if (!fix) return;
    app.applyChanges(fix.label, { enable: fix.enable, disable: fix.disable });
    toast.success(fix.label, { action: { label: "Undo", onClick: () => app.undo() } });
  }

  // The visible title is the switch's accessible name, so it needs an id.
  const uid = $props.id();
  const labelId = `opt-title-${uid}`;
  const detailsId = `opt-details-${uid}`;

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
    // Handled: without this the row re-flashes and steals focus every time it
    // remounts (switching sections and back).
    app.focusParam = null;
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
      <SelectField
        label="{title} value"
        value={value ?? ""}
        options={values.map((v) => ({ value: v, label: v }))}
        onValueChange={(v) => onValue?.(v)}
        size="xs"
        mono
      />
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
      {#if appliedBy}
        <!-- Cleared as soon as the row is edited by hand (see disownParam), so
             the chip never claims a recipe owns a value the user has changed. -->
        <span
          class="inline-flex shrink-0 items-center gap-1 rounded-full px-1.5 py-0.5 text-[10px]"
          style="background: color-mix(in srgb, var(--mauve) 16%, transparent); color: var(--mauve)"
          title="Set by the “{appliedBy}” recipe"
        >
          <Sparkle size={9} weight="fill" />
          {appliedBy}
        </span>
      {/if}
      {#if action}{@render action()}{/if}
      <Badges {requires} {pkg} {gpu} {needs} />
      <button
        type="button"
        onclick={() => (expanded = !expanded)}
        aria-expanded={expanded}
        aria-controls={detailsId}
        aria-label="{expanded ? 'Hide' : 'Show'} details for {title}"
        class="grid size-6 place-items-center rounded-md text-muted transition hover:bg-surface-2 hover:text-text"
      >
        <CaretDown size={13} weight="bold" class="transition {expanded ? 'rotate-180' : ''}" />
      </button>
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

  {#each notices as n (n.id)}
    <p class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 pl-[50px] text-xs leading-snug" style="color: {SEV[n.severity]}">
      <span class="inline-flex items-start gap-1.5">
        <span class="mt-[5px] size-1.5 shrink-0 rounded-full" style="background: {SEV[n.severity]}"></span>
        {n.message}
      </span>
      {#if n.fix}
        <button
          onclick={() => applyFix(n)}
          class="inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-medium transition hover:brightness-110"
          style="background: color-mix(in srgb, {SEV[n.severity]} 18%, transparent)"
        >
          <Wrench size={10} weight="bold" />
          {n.fix.label}
        </button>
      {/if}
    </p>
  {/each}

  {#if expanded}
    <div
      id={detailsId}
      transition:slide={{ duration: 120 }}
      class="ml-[50px] mt-2 grid gap-3 rounded-lg bg-mantle/60 p-3 text-xs sm:grid-cols-[minmax(0,1fr)_14rem]"
    >
      <div class="min-w-0 space-y-2">
        <p class="text-[10px] font-medium uppercase tracking-wider text-muted">About</p>
        <p class="leading-relaxed text-subtext">{details || help || "No further notes for this option."}</p>
        {#if values.length > 1}
          <p class="text-muted">
            Values:
            {#each values as v, i (v)}<code class="font-mono text-text">{v}</code
              >{#if i < values.length - 1}, {/if}{/each}
          </p>
        {/if}
        {#if example}
          <pre class="overflow-x-auto rounded-md bg-mantle p-2 font-mono text-[11px] text-subtext">{example}</pre>
        {/if}
      </div>
      <div class="min-w-0 space-y-2">
        <p class="text-[10px] font-medium uppercase tracking-wider text-muted">Emits</p>
        <p class="break-all font-mono text-[11px] {enabled ? 'text-accent' : 'text-muted'}">{emits}</p>
        {#if defaultValue}
          <p class="text-muted">Default <code class="font-mono text-text">{defaultValue}</code></p>
        {/if}
        <p class="text-muted">
          {#if appliedBy}Set by the “{appliedBy}” recipe{:else if enabled}Turned on by you{:else}Off{/if}
        </p>
        {#if url}
          <button
            type="button"
            onclick={() => openUrl(url)}
            class="inline-flex items-center gap-1 text-blue hover:underline"
          >
            Docs <ArrowSquareOut size={12} weight="bold" />
          </button>
        {/if}
      </div>
    </div>
  {/if}
</div>
