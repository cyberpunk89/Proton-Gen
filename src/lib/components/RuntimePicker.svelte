<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { Combobox } from "bits-ui";
  import { fuzzy } from "$lib/fuzzy";
  import { fly } from "$lib/motion.svelte";
  import { Cpu, CaretUpDown, Check } from "phosphor-svelte";

  let open = $state(false);

  const kindLabel: Record<string, string> = {
    system: "system",
    user: "user",
    valve: "valve",
    auto: "auto-DL",
    // From a dir added under Settings → Paths. Labelled so the user can see
    // their configured directory actually worked.
    custom: "custom",
  };

  // `path` is the identity everywhere else (state.svelte.ts matches on it), so
  // key and value on it too. The old {#each} keyed on display_name, which two
  // runtimes can share.
  let value = $derived(app.selectedRuntime?.path ?? "");

  function onValueChange(next: string) {
    const r = app.runtimes.find((x) => x.path === next);
    if (r) app.setRuntime(r);
  }

  // What the user typed since opening. The input shows the selected runtime's
  // name until they type, so an untouched input filters nothing.
  let search = $state("");

  let shown = $derived.by(() => {
    const q = search.trim();
    if (!q) return app.runtimes;
    return app.runtimes
      .map((r) => ({ r, m: fuzzy(`${r.display_name} ${r.internal_name}`, q, [kindLabel[r.kind] ?? r.kind]) }))
      .filter((x) => x.m !== null)
      .sort((a, b) => b.m!.score - a.m!.score)
      .map((x) => x.r);
  });

  function onOpenChange(o: boolean) {
    if (!o) search = "";
  }
</script>

<!--
  A Combobox so a long list (every GE-Proton a user ever installed) can be
  filtered by typing — fuzzy over display and internal name, plus the kind
  badge. bits-ui supplies the combobox semantics, arrow/Enter/Escape and focus
  restore; filtering is ours. The input is remounted when the selection
  changes from elsewhere (a preset, undo) so it never shows a stale name.
-->
<Combobox.Root
  type="single"
  {value}
  {onValueChange}
  {onOpenChange}
  bind:open
  items={shown.map((r) => ({ value: r.path, label: r.display_name }))}
>
  <div
    class="flex w-full items-center gap-2 rounded-xl border border-border bg-surface-2/60 px-3 py-3 text-left transition hover:border-accent/40 focus-within:border-accent/60"
  >
    <Cpu size={16} class="shrink-0 text-muted" />
    <span class="min-w-0 flex-1">
      <span class="block text-[11px] uppercase tracking-wider text-muted">Proton</span>
      {#key app.selectedRuntime?.path}
        <Combobox.Input
          defaultValue={app.selectedRuntime?.display_name ?? ""}
          placeholder="None — type to search"
          aria-label="Proton runtime"
          oninput={(e) => (search = e.currentTarget.value)}
          onfocus={(e) => e.currentTarget.select()}
          onclick={() => (open = true)}
          class="block w-full truncate border-0 bg-transparent p-0 text-sm text-subtext outline-none placeholder:text-muted"
        />
      {/key}
    </span>
    <Combobox.Trigger aria-label="Show Proton runtimes" class="shrink-0 rounded text-muted hover:text-text">
      <CaretUpDown size={16} />
    </Combobox.Trigger>
  </div>

  <Combobox.Portal>
    <Combobox.Content align="end" sideOffset={8} forceMount>
      <!-- Floating: wrapperProps carries positioning, props the behaviour. -->
      {#snippet child({ props, wrapperProps })}
        <div {...wrapperProps} class="z-50">
          {#if open}
            <div
              {...props}
              transition:fly={{ y: -4, duration: 120 }}
              class="popover max-h-[340px] w-[360px] overflow-y-auto p-1.5"
            >
              {#if app.runtimeWarning}
                <!-- `app.runtimes` always carries a synthetic auto-download
                     entry (see withAutoRuntime in state.svelte.ts), so the
                     {:else} below never actually fires — this is the real,
                     reachable "nothing real was found" notice. -->
                <div
                  class="flex flex-col items-center gap-1 border-b border-border px-3 py-3 text-center"
                >
                  <p class="text-xs text-muted">No Proton runtimes found on this system.</p>
                  <button
                    class="text-xs text-accent underline underline-offset-2 hover:opacity-80"
                    onclick={() => {
                      open = false;
                      app.openSettings("paths");
                    }}
                  >
                    Open Settings → Paths
                  </button>
                </div>
              {/if}
              {#each shown as r (r.path)}
                <Combobox.Item
                  value={r.path}
                  label={r.display_name}
                  class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left data-highlighted:bg-accent/15"
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
                </Combobox.Item>
              {:else}
                <p class="px-3 py-6 text-center text-sm text-muted">
                  {search.trim() ? `No runtime matches “${search.trim()}”.` : "No runtimes found."}
                </p>
              {/each}
            </div>
          {/if}
        </div>
      {/snippet}
    </Combobox.Content>
  </Combobox.Portal>
</Combobox.Root>
