<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { Select } from "bits-ui";
  import { fly } from "$lib/motion.svelte";
  import { Cpu, CaretUpDown, Check } from "phosphor-svelte";

  let open = $state(false);

  const kindLabel: Record<string, string> = {
    system: "system",
    user: "user",
    valve: "valve",
    auto: "auto-DL",
  };

  // `path` is the identity everywhere else (state.svelte.ts matches on it), so
  // key and value on it too. The old {#each} keyed on display_name, which two
  // runtimes can share.
  let value = $derived(app.selectedRuntime?.path ?? "");

  function onValueChange(next: string) {
    const r = app.runtimes.find((x) => x.path === next);
    if (r) app.selectedRuntime = r;
  }
</script>

<!--
  Deliberately Select, not Combobox: this is a select today (no text filter),
  so Select is a pure accessibility fix with no UX change and therefore no
  regression argument. It brings role=combobox semantics, aria-expanded,
  aria-activedescendant, arrow/Enter/Escape, bits-ui's built-in type-ahead,
  and focus restore -- none of which the hand-rolled version had.

  Type-to-filter is a genuine follow-up feature and should be its own issue.
-->
<Select.Root type="single" {value} {onValueChange} bind:open items={app.runtimes.map((r) => ({ value: r.path, label: r.display_name }))}>
  <Select.Trigger
    class="flex w-full items-center gap-2 rounded-xl border border-border bg-surface-2/60 px-3 py-3 text-left transition hover:border-accent/40"
    title="Proton runtime"
    aria-label="Proton runtime"
  >
    <Cpu size={16} class="shrink-0 text-muted" />
    <span class="min-w-0 flex-1">
      <span class="block text-[11px] uppercase tracking-wider text-muted">Proton</span>
      <span class="block truncate text-sm text-subtext">
        {app.selectedRuntime?.display_name ?? "None"}
      </span>
    </span>
    <CaretUpDown size={16} class="shrink-0 text-muted" />
  </Select.Trigger>

  <Select.Portal>
    <Select.Content align="end" sideOffset={8} forceMount>
      <!-- Floating: wrapperProps carries positioning, props the behaviour. -->
      {#snippet child({ props, wrapperProps })}
        <div {...wrapperProps} class="z-50">
          {#if open}
            <div
              {...props}
              transition:fly={{ y: -4, duration: 120 }}
              class="popover max-h-[340px] w-[360px] overflow-y-auto p-1.5"
            >
              {#each app.runtimes as r (r.path)}
                <Select.Item
                  value={r.path}
                  label={r.display_name}
                  class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left data-highlighted:bg-surface-2"
                >
                  {#snippet children({ selected })}
                    {#if selected}
                      <Check size={14} class="shrink-0 text-accent" />
                    {:else}
                      <span class="size-3.5 shrink-0"></span>
                    {/if}
                    <span class="min-w-0 flex-1 truncate text-sm text-text">{r.display_name}</span>
                    <span
                      class="shrink-0 rounded-full px-2 py-0.5 text-[10px]"
                      style="background: color-mix(in srgb, var(--blue) 16%, transparent); color: var(--blue)"
                      >{kindLabel[r.kind] ?? r.kind}</span
                    >
                  {/snippet}
                </Select.Item>
              {:else}
                <p class="px-3 py-6 text-center text-sm text-muted">No runtimes found.</p>
              {/each}
            </div>
          {/if}
        </div>
      {/snippet}
    </Select.Content>
  </Select.Portal>
</Select.Root>
