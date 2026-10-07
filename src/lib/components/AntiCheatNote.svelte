<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { openUrl } from "$lib/util";
  import { ShieldCheck, ShieldWarning, ArrowSquareOut } from "phosphor-svelte";

  /**
   * AreWeAntiCheatYet's verdict for the selected game, when it lists one.
   * "Denied"/"Broken" is the one thing worth knowing before any tuning: no
   * launch option makes a kernel anti-cheat that blocks Linux run.
   */
  let ac = $derived(app.selectedAppId == null ? undefined : app.anticheatCache[String(app.selectedAppId)]);
  let bad = $derived(ac ? ac.status === "Denied" || ac.status === "Broken" : false);
  let gplasync = $derived(app.env["PROTON_DXVK_GPLASYNC"]?.enabled ?? false);

  const MEANING: Record<string, string> = {
    Supported: "works — the developer enabled Linux support",
    Running: "runs, per player reports",
    Planned: "Linux support is planned",
    Broken: "broken on Linux right now",
    Denied: "the developer blocks Linux — it won't run under Proton",
  };
</script>

{#if app.store.anticheat_check && ac}
  <div
    class="flex items-start gap-2 rounded-xl border px-3 py-2 text-xs"
    style="border-color: color-mix(in srgb, {bad ? 'var(--red)' : 'var(--green)'} 35%, transparent); background: color-mix(in srgb, {bad
      ? 'var(--red)'
      : 'var(--green)'} 7%, transparent)"
    role={bad ? "alert" : undefined}
  >
    {#if bad}
      <ShieldWarning size={15} weight="fill" class="mt-0.5 shrink-0 text-red" />
    {:else}
      <ShieldCheck size={15} weight="fill" class="mt-0.5 shrink-0 text-green" />
    {/if}
    <span class="min-w-0 flex-1 text-subtext">
      <span class="font-medium text-text">{ac.anticheats.join(", ")}</span>:
      {MEANING[ac.status] ?? ac.status}.
      {#if gplasync && !bad}
        Leave <span class="font-mono">PROTON_DXVK_GPLASYNC</span> off for this one — kernel anti-cheat
        can flag it.
      {/if}
      {#if ac.updated}<span class="text-muted"> (as of {ac.updated})</span>{/if}
    </span>
    <button
      onclick={() => openUrl(ac!.url)}
      class="inline-flex shrink-0 items-center gap-1 text-muted hover:text-text"
      aria-label="Open on AreWeAntiCheatYet"><ArrowSquareOut size={13} /></button
    >
  </div>
{/if}
