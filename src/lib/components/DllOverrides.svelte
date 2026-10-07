<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import SelectField from "./SelectField.svelte";
  import { Plus, X } from "phosphor-svelte";
  import { COMMON, MODES, buildOverrides, parseOverrides, type Override } from "$lib/dlloverrides";

  let { onapply }: { onapply?: () => void } = $props();

  // Seeded from the current WINEDLLOVERRIDES each time the dialog opens.
  let rows = $state<Override[]>([]);
  $effect(() => {
    if (!app.dllBuilderOpen) return;
    const cur = app.env["WINEDLLOVERRIDES"];
    rows = parseOverrides(cur?.enabled ? cur.value : "");
  });

  let value = $derived(buildOverrides(rows));
  const has = (dlls: string) => rows.some((r) => r.raw === undefined && r.dlls === dlls);

  function add(dlls = "", mode = "n,b") {
    rows.push({ dlls, mode });
  }

  function apply() {
    if (value) app.applyChanges("set DLL overrides", { enable: [["WINEDLLOVERRIDES", value]] });
    else app.applyChanges("clear DLL overrides", { disable: ["WINEDLLOVERRIDES"] });
    toast.success(value ? "DLL overrides set" : "DLL overrides cleared", {
      action: { label: "Undo", onClick: () => app.undo() },
    });
    onapply?.();
  }
</script>

<div class="space-y-4">
  <p class="text-xs text-muted">
    Tell Wine to load a game's own copy of a DLL (native) instead of Wine's (builtin) — what
    mods, OptiScaler and ReShade need — or to skip a DLL entirely.
  </p>

  <div class="flex flex-wrap gap-1.5">
    {#each COMMON as c (c.dlls)}
      <button
        onclick={() => add(c.dlls, c.mode)}
        disabled={has(c.dlls)}
        title={c.why}
        class="rounded-full border border-border px-2.5 py-1 font-mono text-[11px] text-subtext transition hover:border-accent/50 disabled:opacity-40"
        >+ {c.dlls}={c.mode}</button
      >
    {/each}
  </div>

  <div class="space-y-1.5">
    {#each rows as r, i (i)}
      <div class="flex items-center gap-2">
        {#if r.raw !== undefined}
          <span class="flex-1 truncate rounded-lg border border-border bg-surface-2/60 px-2 py-1 font-mono text-xs text-muted" title="Kept as written — not in a form this editor models"
            >{r.raw}</span
          >
        {:else}
          <input
            value={r.dlls}
            oninput={(e) => (r.dlls = e.currentTarget.value)}
            placeholder="dxgi or d3d11,dxgi"
            aria-label="DLL names"
            class="min-w-0 flex-1 rounded-lg border border-border bg-surface-2 px-2 py-1 font-mono text-xs text-text outline-none focus:border-accent"
          />
          <SelectField label="Load mode" value={r.mode} options={MODES} onValueChange={(v) => (r.mode = v)} width="w-48" size="xs" />
        {/if}
        <button
          onclick={() => rows.splice(i, 1)}
          aria-label="Remove override"
          class="rounded p-1 text-muted transition hover:text-red"><X size={13} /></button
        >
      </div>
    {:else}
      <p class="text-xs text-muted">No overrides — Wine decides for every DLL.</p>
    {/each}
    <button
      onclick={() => add()}
      class="inline-flex items-center gap-1 text-xs text-accent transition hover:opacity-80"
      ><Plus size={12} /> Add override</button
    >
  </div>

  <p class="break-all rounded-lg bg-surface-2/60 px-2.5 py-2 font-mono text-xs text-subtext">
    WINEDLLOVERRIDES={value ? `"${value}"` : "(unset)"}
  </p>

  <div class="flex justify-end">
    <button
      onclick={apply}
      class="rounded-lg px-3 py-1.5 text-sm font-medium transition active:scale-95"
      style="background: var(--accent); color: var(--on-accent)">Apply to command</button
    >
  </div>
</div>
