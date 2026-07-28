<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import RuntimePicker from "./RuntimePicker.svelte";
  import UmuFields from "./UmuFields.svelte";
  import ProtonDbChip from "./ProtonDbChip.svelte";
  import { DownloadSimple, Cpu } from "phosphor-svelte";

  let selectedGame = $derived(
    app.selectedAppId == null
      ? null
      : app.games.find((g) => g.app_id === app.selectedAppId) ?? null,
  );
  let isSteam = $derived(selectedGame?.source === "steam");

  let currentOpts = $derived(
    isSteam && app.selectedAppId != null
      ? app.launchOptions[String(app.selectedAppId)] ?? ""
      : "",
  );
  let currentTool = $derived(
    isSteam && app.selectedAppId != null
      ? app.compatTools[String(app.selectedAppId)] ?? ""
      : "",
  );

  async function loadCurrent() {
    if (!currentOpts) return;
    await app.importCommand(currentOpts);
    toast.success("Loaded current launch options");
  }
</script>

<section class="card space-y-4 p-4">
  <div class="flex items-center gap-2">
    <Cpu size={18} class="text-accent" />
    <h2 class="text-sm font-medium tracking-wide text-text">Game &amp; runtime</h2>
  </div>

  <RuntimePicker />

  {#if app.umu}
    <UmuFields />
  {/if}

  {#if isSteam}
    <div class="flex flex-wrap items-center gap-3">
      <ProtonDbChip />
      {#if currentOpts || currentTool}
        <div class="ml-auto flex items-center gap-2 text-xs text-muted">
          {#if currentTool}
            <span class="font-mono">Proton={currentTool}</span>
          {/if}
          {#if currentOpts}
            <span class="max-w-[280px] truncate font-mono" title={currentOpts}
              >{currentOpts}</span
            >
            <button
              onclick={loadCurrent}
              class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/60 px-2.5 py-1.5 text-subtext transition hover:border-accent/50"
            >
              <DownloadSimple size={13} /> Load current
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
    <label class="block">
      <span class="mb-1 block text-[11px] uppercase tracking-wider text-muted"
        >Game arguments</span
      >
      <input
        bind:value={app.gameArgs}
        placeholder="-windowed -novid"
        class="w-full rounded-lg border border-border bg-surface-2 px-2.5 py-2 font-mono text-xs text-text outline-none focus:border-accent"
      />
    </label>
    <label class="block">
      <span class="mb-1 block text-[11px] uppercase tracking-wider text-muted"
        >Custom env</span
      >
      <input
        bind:value={app.extraEnv}
        placeholder="KEY=VALUE KEY2=VALUE2"
        class="w-full rounded-lg border border-border bg-surface-2 px-2.5 py-2 font-mono text-xs text-text outline-none focus:border-accent"
      />
    </label>
  </div>
</section>
