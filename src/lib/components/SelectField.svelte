<script lang="ts">
  import { Select } from "bits-ui";
  import { fly } from "$lib/motion.svelte";
  import { CaretUpDown, Check } from "phosphor-svelte";

  /**
   * The app's one themed dropdown, replacing the native `<select>`. WebKitGTK
   * draws a native select's open list with the GTK system theme, ignoring the
   * page, so on a dark palette it opened as a bright, low-contrast sheet. This
   * renders the list itself, on the same `popover` surface as RuntimePicker.
   */
  let {
    value,
    options,
    onValueChange,
    label,
    width = "w-40",
    size = "sm",
    mono = false,
  }: {
    value: string;
    options: { value: string; label: string }[];
    onValueChange: (v: string) => void;
    /** Accessible name — the trigger is a button, not a labelled form control. */
    label: string;
    /** Tailwind width class for the trigger. */
    width?: string;
    size?: "xs" | "sm";
    mono?: boolean;
  } = $props();

  let open = $state(false);

  // bits-ui treats "" as "nothing selected", but every builder here uses "" to
  // mean "leave it at the tool's default" — a real, selectable option. Map it
  // to a sentinel on the way in and back out.
  const EMPTY = "\u0000default";
  const enc = (v: string) => (v === "" ? EMPTY : v);
  const dec = (v: string) => (v === EMPTY ? "" : v);

  let items = $derived(options.map((o) => ({ value: enc(o.value), label: o.label })));
  let current = $derived(options.find((o) => o.value === value));
  let text = $derived(size === "xs" ? "text-xs" : "text-sm");
</script>

<Select.Root
  type="single"
  value={enc(value)}
  onValueChange={(v) => onValueChange(dec(v))}
  bind:open
  {items}
>
  <Select.Trigger
    aria-label={label}
    class="flex {width} shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-2 py-1 text-left {text} text-text outline-none transition hover:border-accent/40 focus-visible:border-accent data-[state=open]:border-accent"
  >
    <span class="min-w-0 flex-1 truncate {mono ? 'font-mono' : ''}">
      <!-- A value loaded from a hand-edited file may not be in the list;
           show it verbatim rather than silently naming a different option. -->
      {current?.label ?? (value || "—")}
    </span>
    <CaretUpDown size={12} class="shrink-0 text-muted" />
  </Select.Trigger>

  <Select.Portal>
    <Select.Content sideOffset={6} forceMount>
      {#snippet child({ props, wrapperProps })}
        <div {...wrapperProps}>
          {#if open}
            <!-- z-[120]: bits-ui copies this element's z-index onto the
                 positioning wrapper, and the builders that use this sit in
                 z-[100] dialogs. -->
            <div
              {...props}
              transition:fly={{ y: -4, duration: 120 }}
              class="popover z-[120] max-h-[300px] min-w-[var(--bits-floating-anchor-width)] overflow-y-auto p-1"
            >
              {#each items as it (it.value)}
                <Select.Item
                  value={it.value}
                  label={it.label}
                  class="flex w-full cursor-default items-center gap-2 rounded-md px-2 py-1.5 {text} text-subtext transition-colors data-highlighted:bg-accent/15 data-highlighted:text-text data-[selected]:text-text"
                >
                  {#snippet children({ selected })}
                    {#if selected}
                      <Check size={12} class="shrink-0 text-accent" />
                    {:else}
                      <span class="size-3 shrink-0"></span>
                    {/if}
                    <span class="min-w-0 flex-1 truncate {mono ? 'font-mono' : ''}">{it.label}</span>
                  {/snippet}
                </Select.Item>
              {/each}
            </div>
          {/if}
        </div>
      {/snippet}
    </Select.Content>
  </Select.Portal>
</Select.Root>
