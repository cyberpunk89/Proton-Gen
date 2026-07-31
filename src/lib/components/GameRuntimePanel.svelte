<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import RuntimePicker from "./RuntimePicker.svelte";
  import UmuFields from "./UmuFields.svelte";
  import ProtonDbChip from "./ProtonDbChip.svelte";
  import SyncPill from "./SyncPill.svelte";
  import OpenInSteam from "./OpenInSteam.svelte";
  import { DownloadSimple, Cpu, CheckCircle, WarningCircle } from "phosphor-svelte";

  let isSteam = $derived(app.selectedGame?.source === "steam");
  let mismatch = $derived(app.runtimeMismatch);

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

<!-- `@container` so the SyncPill's responsive label has a container to query here
     too. Without one the query never matches and the pill would be stuck in its
     compact form in a panel that has plenty of room for the full sentence. -->
<section class="@container card space-y-4 p-4">
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
      <div class="ml-auto"><SyncPill /></div>
    </div>

    <!--
      What Steam has right now. Was a 280px-truncated span with only a `title`
      tooltip, which made a long launch string unreadable exactly when you needed
      to read it — hence the wrapping <pre>.
    -->
    <div class="space-y-2 rounded-xl border border-border/60 bg-surface-2/40 p-3">
      <p class="text-[11px] font-medium uppercase tracking-wider text-muted">
        Currently in Steam
      </p>

      {#if currentOpts}
        <pre
          class="whitespace-pre-wrap break-words font-mono text-xs text-subtext">{currentOpts}</pre>
      {:else}
        <p class="text-xs text-muted">No launch options set.</p>
      {/if}

      <!-- Agreement or disagreement, not a bare echo of a value. -->
      <p class="flex items-start gap-1.5 text-xs">
        {#if !app.runtimeComparable}
          <span class="text-muted">
            Proton: <code class="font-mono">{currentTool || "not set"}</code>
          </span>
        {:else if mismatch}
          <WarningCircle size={13} weight="fill" class="mt-0.5 shrink-0 text-peach" />
          <span class="text-subtext">
            {#if mismatch.steam}
              Steam is set to <code class="font-mono text-text">{mismatch.steam}</code>
              — change it to <code class="font-mono text-text">{mismatch.wanted}</code>.
            {:else}
              Steam has no Proton set — choose
              <code class="font-mono text-text">{mismatch.wanted}</code>.
            {/if}
          </span>
        {:else}
          <CheckCircle size={13} weight="fill" class="mt-0.5 shrink-0 text-green" />
          <span class="text-subtext">
            Steam is already set to
            <code class="font-mono text-text">{currentTool}</code>.
          </span>
        {/if}
      </p>

      <div class="flex flex-wrap items-center gap-2">
        <OpenInSteam />
        {#if currentOpts}
          <button
            onclick={loadCurrent}
            class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/60 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50"
          >
            <DownloadSimple size={13} /> Load current
          </button>
        {/if}
      </div>
    </div>
  {/if}

  <!--
    These two are `value` + `oninput` rather than `bind:value`, so the handler
    can both assign and name the undo entry. Do not "simplify" to `bind:value`
    with a separate `oninput`: Svelte compiles that oninput to a *delegated*
    handler which never fires once bind_value owns the element's input
    listener, so the history entry silently degrades to the generic "edit".
  -->
  <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
    <label class="block">
      <span class="mb-1 block text-[11px] uppercase tracking-wider text-muted"
        >Game arguments</span
      >
      <input
        value={app.gameArgs}
        oninput={(e) => {
          app.gameArgs = e.currentTarget.value;
          app.noteEdit("set game arguments");
        }}
        placeholder="-windowed -novid"
        class="w-full rounded-lg border border-border bg-surface-2 px-2.5 py-2 font-mono text-xs text-text outline-none focus:border-accent"
      />
    </label>
    <label class="block">
      <span class="mb-1 block text-[11px] uppercase tracking-wider text-muted"
        >Custom env</span
      >
      <input
        value={app.extraEnv}
        oninput={(e) => {
          app.extraEnv = e.currentTarget.value;
          app.noteEdit("set custom env");
        }}
        placeholder="KEY=VALUE KEY2=VALUE2"
        class="w-full rounded-lg border border-border bg-surface-2 px-2.5 py-2 font-mono text-xs text-text outline-none focus:border-accent"
      />
    </label>
  </div>
</section>
