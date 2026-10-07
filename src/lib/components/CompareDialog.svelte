<script lang="ts">
  import { app } from "$lib/state.svelte";
  import Dialog from "./Dialog.svelte";
  import SelectField from "./SelectField.svelte";
  import { compareConfigs, type Change } from "$lib/compare";
  import type { Config } from "$lib/types";
  import { ArrowsLeftRight } from "phosphor-svelte";

  /**
   * Compare two configs: the builder's current one, any saved preset, or any
   * game's saved tuning. Read-only — it only says what differs. Root-mounted
   * (opened from the Presets popover) so a popover closing can't unmount an
   * open modal (#63).
   */
  type Source = { value: string; label: string; config: () => Config | null };

  let sources = $derived.by((): Source[] => {
    const out: Source[] = [
      {
        value: "current",
        label: `Current builder${app.selectedGameName ? ` (${app.selectedGameName})` : ""}`,
        config: () => app.toConfig(),
      },
    ];
    for (const p of app.store.presets) {
      out.push({ value: `preset:${p.name}`, label: `Preset: ${p.name}`, config: () => p.config });
    }
    for (const [id, cfg] of Object.entries(app.store.game_memory)) {
      const g = app.games.find((x) => String(x.app_id) === id);
      if (!g) continue;
      out.push({ value: `game:${id}`, label: `Game: ${g.name}`, config: () => cfg });
    }
    return out;
  });

  let a = $state("current");
  let b = $state("");
  $effect(() => {
    if (!app.compareOpen) return;
    if (!sources.some((s) => s.value === a)) a = "current";
    if (!b || !sources.some((s) => s.value === b) || b === a) {
      b = sources.find((s) => s.value !== a)?.value ?? "";
    }
  });

  let cfgA = $derived(sources.find((s) => s.value === a)?.config() ?? null);
  let cfgB = $derived(sources.find((s) => s.value === b)?.config() ?? null);
  let rows = $derived(cfgA && cfgB ? compareConfigs(cfgA, cfgB) : []);

  const MARK: Record<Change, { sym: string; color: string; sr: string }> = {
    same: { sym: "=", color: "var(--muted)", sr: "same" },
    changed: { sym: "≠", color: "var(--yellow)", sr: "different" },
    "only-a": { sym: "A", color: "var(--peach)", sr: "only in A" },
    "only-b": { sym: "B", color: "var(--blue)", sr: "only in B" },
  };

  function swap() {
    [a, b] = [b, a];
  }
</script>

<Dialog bind:open={app.compareOpen} title="Compare" subtitle="What differs between two configs." width="48rem">
  <div class="space-y-3">
    <div class="flex flex-wrap items-center gap-2">
      <span class="text-xs font-medium text-muted">A</span>
      <SelectField label="Config A" value={a} options={sources} onValueChange={(v) => (a = v)} width="w-64" />
      <button onclick={swap} aria-label="Swap A and B" class="rounded p-1 text-muted hover:text-text"
        ><ArrowsLeftRight size={15} /></button
      >
      <span class="text-xs font-medium text-muted">B</span>
      <SelectField label="Config B" value={b} options={sources} onValueChange={(v) => (b = v)} width="w-64" />
    </div>

    {#if sources.length < 2}
      <p class="py-6 text-center text-sm text-muted">Save a preset or tune a game to have something to compare.</p>
    {:else if rows.length === 0}
      <p class="py-6 text-center text-sm text-muted">These two are the same.</p>
    {:else}
      <table class="w-full table-fixed text-xs">
        <thead>
          <tr class="text-left text-[11px] uppercase tracking-wider text-muted">
            <th class="w-6"></th>
            <th class="w-1/3 pb-1.5 font-medium">Setting</th>
            <th class="pb-1.5 font-medium">A</th>
            <th class="pb-1.5 font-medium">B</th>
          </tr>
        </thead>
        <tbody>
          {#each rows as r (r.kind + r.label)}
            <tr class="border-t border-border/50 align-top">
              <td class="py-1.5 font-mono font-bold" style="color: {MARK[r.change].color}"
                ><span aria-hidden="true">{MARK[r.change].sym}</span><span class="sr-only">{MARK[r.change].sr}</span></td
              >
              <td class="break-all py-1.5 {r.kind === 'field' ? 'text-subtext' : 'font-mono text-text'}">{r.label}</td>
              <td class="break-all py-1.5 font-mono {r.a === null ? 'text-muted' : 'text-subtext'}">{r.a ?? "—"}</td>
              <td class="break-all py-1.5 font-mono {r.b === null ? 'text-muted' : 'text-subtext'}">{r.b ?? "—"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="text-[11px] text-muted">
        {rows.length} difference{rows.length === 1 ? "" : "s"}. Launch target (exe, prefix) is
        left out — it belongs to the game, not the tuning.
      </p>
    {/if}
  </div>
</Dialog>
