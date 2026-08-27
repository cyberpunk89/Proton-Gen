<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import {
    DX12_UPSCALERS,
    DX11_UPSCALERS,
    VULKAN_UPSCALERS,
    FG_INPUTS,
    FG_OUTPUTS,
    SHARPEN_SHADERS,
    OPTI_FIXES,
    PROXY_DLLS,
    parseOptiScaler,
    buildOptiScaler,
    type OptiScalerConfig,
  } from "$lib/optiscaler";
  import Dialog from "./Dialog.svelte";
  import { untrack } from "svelte";
  import {
    ArrowSquareOut,
    ArrowClockwise,
    CloudArrowDown,
    CheckCircle,
  } from "phosphor-svelte";

  let { onapply }: { onapply?: () => void } = $props();

  // -------------------------- OptiScaler upgrade --------------------------
  // Fetch the latest OptiScaler release from GitHub and extract it into the
  // selected game's folder — the one action here that writes into a game's
  // own directory rather than just building a command string (see
  // optiscaler_upgrade.rs's doc comment for the full rationale). Only offered
  // when the folder already shows signs of an OptiScaler install: this is a
  // refresh, never an injection into a game that isn't using it.
  let appId = $derived(app.selectedAppId);
  $effect(() => {
    if (appId != null) app.requestOptiscalerStatus(appId);
  });
  let status = $derived(appId == null ? undefined : app.optiscalerStatusFor(appId));
  let statusLoading = $derived(appId != null && app.optiscalerStatusLoading[String(appId)] === true);

  let confirmOpen = $state(false);

  function openConfirm() {
    app.requestOptiscalerLatest();
    confirmOpen = true;
  }

  async function doFetch() {
    if (appId == null) return;
    try {
      const result = await app.fetchOptiscalerUpgrade(appId);
      confirmOpen = false;
      const kept = result.ini_preserved ? " — kept your existing OptiScaler.ini" : "";
      toast.success(`Installed OptiScaler ${result.tag}: ${result.files_written} files written${kept}`);
    } catch (e) {
      toast.error(`Couldn't fetch OptiScaler: ${e}`, { ms: 6000 });
    }
  }

  // Local $state rather than writing straight through to the store: the user
  // must be able to build a config and abandon it (same rationale as MangoHud).
  const seed = parseOptiScaler(app.env["PROTON_OPTISCALER_CONFIG"]?.value ?? "");

  let c = $state<OptiScalerConfig>(seed);

  // The proxy DLL is its own env var, not an ini key, so it is tracked beside
  // the config rather than inside it.
  let proxy = $state(
    app.env["PROTON_OPTISCALER_NAME"]?.enabled
      ? (app.env["PROTON_OPTISCALER_NAME"]?.value ?? "")
      : "",
  );

  let config = $derived(buildOptiScaler(c));

  let fixCount = $derived(OPTI_FIXES.filter((f) => c.fixes[f.id]).length);

  // Re-seed when the env changes underneath us — an undo, a recipe, a preset, or
  // a hand edit on the PROTON_OPTISCALER_CONFIG row while this panel is open.
  // `config` is read untracked so typing here doesn't re-run the effect and stomp
  // the very edit that caused it (see MangoHud.svelte for the full reasoning).
  let sourceConfig = $derived(app.env["PROTON_OPTISCALER_CONFIG"]?.value ?? "");
  $effect(() => {
    const src = sourceConfig;
    if (src === untrack(() => config)) return;
    c = parseOptiScaler(src);
  });

  // Same treatment for the proxy DLL, which lives on its own row.
  let sourceProxy = $derived(
    app.env["PROTON_OPTISCALER_NAME"]?.enabled
      ? (app.env["PROTON_OPTISCALER_NAME"]?.value ?? "")
      : "",
  );
  $effect(() => {
    const src = sourceProxy;
    if (src === untrack(() => proxy)) return;
    proxy = src;
  });

  function apply() {
    app.applyOptiScaler(config, proxy);
    toast.success("OptiScaler config applied");
    onapply?.();
  }
</script>

<div class="space-y-3">
  {#if appId != null}
    <div class="space-y-2 rounded-lg border border-border/60 p-3">
      <p class="text-[11px] font-medium uppercase tracking-wider text-muted">
        Upgrade the installed OptiScaler build
      </p>
      {#if statusLoading || !status}
        <p class="flex items-center gap-1.5 text-xs text-muted">
          <ArrowClockwise size={12} class="animate-spin" /> Checking this game's folder…
        </p>
      {:else if !status.install_dir}
        <p class="text-xs text-muted">
          This game's install folder couldn't be resolved, so this isn't available here.
        </p>
      {:else if !status.found}
        <p class="text-xs text-muted">
          No OptiScaler install detected in <span class="font-mono">{status.install_dir}</span> yet —
          nothing to upgrade. Enable OptiScaler above and launch the game once first.
        </p>
      {:else}
        <p class="text-xs text-subtext">
          Detected in <span class="font-mono">{status.install_dir}</span>.
        </p>
        <button
          onclick={openConfirm}
          class="inline-flex items-center gap-1.5 rounded-lg border border-accent/40 bg-accent/5 px-2.5 py-1 text-xs font-medium text-accent transition hover:bg-accent/10"
        >
          <CloudArrowDown size={13} /> Fetch latest OptiScaler build…
        </button>
      {/if}
    </div>
  {/if}

  <p class="text-xs text-muted">
    Compose OptiScaler.ini settings, then apply them to <span class="font-mono"
      >PROTON_OPTISCALER_CONFIG</span
    >. Only the options you change are written; the rest stay at OptiScaler's defaults.
  </p>

  <div class="grid gap-5 md:grid-cols-2">
    <div class="space-y-3">
      <!-- Upscaler selection -->
      <div class="space-y-2">
        <p class="text-[11px] font-medium uppercase tracking-wider text-muted">Upscaler</p>
        {@render pick("DirectX 12", DX12_UPSCALERS, () => c.dx12Upscaler, (v) => (c.dx12Upscaler = v))}
        {@render pick("DirectX 11", DX11_UPSCALERS, () => c.dx11Upscaler, (v) => (c.dx11Upscaler = v))}
        {@render pick("Vulkan", VULKAN_UPSCALERS, () => c.vulkanUpscaler, (v) => (c.vulkanUpscaler = v))}
        {@render pick("Inject as", PROXY_DLLS, () => proxy, (v) => (proxy = v))}
        <p class="text-[11px] leading-snug text-muted">
          Change “Inject as” if OptiScaler never loads at all — some games already own
          <span class="font-mono">dxgi.dll</span>.
        </p>
      </div>

      <!-- Output scaling -->
      <div class="space-y-2 border-t border-border/60 pt-3">
        <label class="flex items-center gap-2">
          <input type="checkbox" bind:checked={c.outputScalingOn} class="accent-[var(--accent)]" />
          <span class="text-sm text-subtext">Output scaling</span>
          <input
            type="range"
            min="0.5"
            max="3"
            step="0.1"
            bind:value={c.outputScalingMult}
            disabled={!c.outputScalingOn}
            aria-label="Output scaling multiplier"
            class="ml-auto w-28 accent-[var(--accent)] disabled:opacity-40"
          />
          <span class="w-8 text-right font-mono text-xs text-muted">{c.outputScalingMult}×</span>
        </label>
      </div>

      <!-- Sharpening (RCAS) -->
      <div class="space-y-2 border-t border-border/60 pt-3">
        <label class="flex items-center gap-2">
          <input type="checkbox" bind:checked={c.sharpenOn} class="accent-[var(--accent)]" />
          <span class="text-sm text-subtext">Sharpening</span>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            bind:value={c.sharpness}
            disabled={!c.sharpenOn}
            aria-label="Sharpness amount"
            class="ml-auto w-28 accent-[var(--accent)] disabled:opacity-40"
          />
          <span class="w-8 text-right font-mono text-xs text-muted">{c.sharpness}</span>
        </label>
        {#if c.sharpenOn}
          <label class="flex items-center justify-between gap-2 pl-6">
            <span class="text-sm text-subtext">Filter</span>
            <select
              bind:value={c.sharpenShader}
              class="w-40 rounded-lg border border-border bg-surface-2 px-2 py-1 text-sm text-text outline-none focus:border-accent"
            >
              {#each SHARPEN_SHADERS as s (s.value)}
                <option value={s.value}>{s.label}</option>
              {/each}
            </select>
          </label>
        {/if}
      </div>

      <!-- Frame generation -->
      <div class="space-y-2 border-t border-border/60 pt-3">
        <label class="flex items-center gap-2">
          <input type="checkbox" bind:checked={c.frameGenOn} class="accent-[var(--accent)]" />
          <span class="text-sm text-subtext">Frame generation (OptiFG)</span>
        </label>
        {#if c.frameGenOn}
          <label class="flex items-center justify-between gap-2 pl-6">
            <span class="text-sm text-subtext">Input</span>
            <select
              bind:value={c.fgInput}
              class="w-40 rounded-lg border border-border bg-surface-2 px-2 py-1 text-sm text-text outline-none focus:border-accent"
            >
              {#each FG_INPUTS as f (f.value)}
                <option value={f.value}>{f.label}</option>
              {/each}
            </select>
          </label>
          <label class="flex items-center justify-between gap-2 pl-6">
            <span class="text-sm text-subtext">Output</span>
            <select
              bind:value={c.fgOutput}
              class="w-40 rounded-lg border border-border bg-surface-2 px-2 py-1 text-sm text-text outline-none focus:border-accent"
            >
              {#each FG_OUTPUTS as f (f.value)}
                <option value={f.value}>{f.label}</option>
              {/each}
            </select>
          </label>
        {/if}
      </div>

      <!-- DLSS render preset (ray-reconstruction / quality tuning) -->
      <div class="space-y-2 border-t border-border/60 pt-3">
        <label class="flex items-center gap-2">
          <input type="checkbox" bind:checked={c.dlssPresetOn} class="accent-[var(--accent)]" />
          <span class="text-sm text-subtext">Override DLSS render preset</span>
        </label>
        {#if c.dlssPresetOn}
          <label class="flex items-center justify-between gap-2 pl-6">
            <span class="text-sm text-subtext">Preset (0–15)</span>
            <input
              bind:value={c.dlssPreset}
              inputmode="numeric"
              placeholder="0"
              class="w-20 rounded-lg border border-border bg-surface-2 px-2 py-1 text-sm text-text outline-none focus:border-accent"
            />
          </label>
        {/if}
      </div>

      <!-- GPU spoofing: unlock DLSS/RT on Radeon -->
      <div class="space-y-2 border-t border-border/60 pt-3">
        <label class="flex gap-2">
          <input type="checkbox" bind:checked={c.spoofUnlock} class="mt-0.5 shrink-0 accent-[var(--accent)]" />
          <span class="min-w-0">
            <span class="block text-sm leading-snug text-subtext">Unlock DLSS / RT on Radeon</span>
            <span class="block text-[11px] leading-snug text-muted">
              Streamline GPU spoofing, so RE Engine / Streamline games expose their DLSS and
              ray-tracing options for OptiScaler to translate. On by default upstream — set it here
              when a game hides them.
            </span>
          </span>
        </label>
      </div>
    </div>

    <div class="space-y-4">
      <!--
        Compatibility fixes, labelled by symptom. These are the settings people
        come here for when a game misbehaves, and none of them is guessable from
        its ini key — so the key is not shown at all; it is in the Result pane
        below for anyone who wants to check.
      -->
      <div class="space-y-2">
        <p class="text-[11px] font-medium uppercase tracking-wider text-muted">
          Compatibility fixes{fixCount ? ` · ${fixCount} on` : ""}
        </p>
        <p class="text-[11px] leading-snug text-muted">
          Only turn these on for a problem you're actually seeing — each one trades
          something away.
        </p>
        {#each OPTI_FIXES as f (f.id)}
          <label class="flex gap-2">
            <input
              type="checkbox"
              checked={!!c.fixes[f.id]}
              onchange={(e) => (c.fixes[f.id] = e.currentTarget.checked)}
              class="mt-0.5 shrink-0 accent-[var(--accent)]"
            />
            <span class="min-w-0">
              <span class="block text-sm leading-snug text-subtext">{f.label}</span>
              <span class="block text-[11px] leading-snug text-muted">{f.note}</span>
            </span>
          </label>
        {/each}
      </div>

      <!-- Resulting string -->
      <div class="space-y-1.5 border-t border-border/60 pt-3">
        <p class="text-[11px] font-medium uppercase tracking-wider text-muted">Result</p>
        <p class="overflow-x-auto rounded-lg bg-mantle p-2 font-mono text-xs break-all text-muted">
          PROTON_OPTISCALER_CONFIG={config || "none"}
        </p>
        <p class="text-[11px] leading-snug text-muted">
          Applying also enables <span class="font-mono">PROTON_USE_OPTISCALER</span>, so the
          injected config takes effect.
        </p>
        {#if c.passthrough.length}
          <p class="text-[11px] leading-snug text-muted">
            {c.passthrough.length} setting{c.passthrough.length === 1 ? "" : "s"} this builder doesn't
            model {c.passthrough.length === 1 ? "is" : "are"} kept as-is.
          </p>
        {/if}
      </div>
    </div>
  </div>

  <div class="sticky bottom-0 -mx-1 flex justify-end bg-surface-solid px-1 pb-1 pt-2">
    <button
      onclick={apply}
      class="rounded-lg px-3 py-1.5 text-sm font-medium transition active:scale-95"
      style="background: var(--accent); color: var(--on-accent)">Apply OptiScaler config</button
    >
  </div>
</div>

<Dialog bind:open={confirmOpen} title="Fetch latest OptiScaler build" width="26rem">
  <div class="space-y-3 text-sm">
    {#if app.optiscalerLatestLoading}
      <p class="flex items-center gap-1.5 text-xs text-muted">
        <ArrowClockwise size={12} class="animate-spin" /> Checking the latest release…
      </p>
    {:else if app.optiscalerLatestError}
      <p class="text-xs text-red">Couldn't reach GitHub: {app.optiscalerLatestError}</p>
    {:else if app.optiscalerLatest}
      <div class="space-y-1.5 rounded-lg border border-border/60 bg-surface-2/60 p-3 text-xs">
        <div class="flex items-center justify-between gap-2">
          <span class="text-muted">Source</span>
          <a
            href={app.optiscalerLatest.html_url}
            target="_blank"
            rel="noopener noreferrer"
            class="inline-flex items-center gap-1 font-medium text-accent hover:underline"
          >
            optiscaler/OptiScaler {app.optiscalerLatest.tag} <ArrowSquareOut size={11} />
          </a>
        </div>
        <div class="flex items-center justify-between gap-2">
          <span class="text-muted">Asset</span>
          <span class="truncate font-mono text-subtext">{app.optiscalerLatest.asset_name}</span>
        </div>
        <div class="flex items-center justify-between gap-2">
          <span class="text-muted">Destination</span>
          <span class="truncate font-mono text-subtext">{status?.install_dir}</span>
        </div>
      </div>
      <p class="text-xs leading-snug text-muted">
        Downloads that archive and extracts every file it contains into the folder above,
        overwriting anything with the same name — <span class="font-medium text-subtext"
          >except an existing <span class="font-mono">OptiScaler.ini</span>, which is left
          untouched.</span
        > No checksum is published for this release; integrity rests on HTTPS + fetching directly
        from the project's own GitHub Releases.
      </p>
    {/if}

    <div class="flex justify-end gap-2 pt-1">
      <button
        onclick={() => (confirmOpen = false)}
        disabled={app.optiscalerFetchBusy}
        class="rounded-lg px-3 py-1.5 text-xs text-muted hover:text-text disabled:opacity-40"
      >
        Cancel
      </button>
      <button
        onclick={doFetch}
        disabled={!app.optiscalerLatest || app.optiscalerFetchBusy}
        class="inline-flex items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-on-accent transition hover:opacity-90 disabled:opacity-40"
      >
        {#if app.optiscalerFetchBusy}
          <ArrowClockwise size={13} class="animate-spin" /> Installing…
        {:else}
          <CheckCircle size={13} /> Fetch &amp; install
        {/if}
      </button>
    </div>
  </div>
</Dialog>

{#snippet pick(
  label: string,
  choices: { value: string; label: string }[],
  get: () => string,
  set: (v: string) => void,
)}
  <label class="flex items-center justify-between gap-2">
    <span class="text-sm text-subtext">{label}</span>
    <select
      value={get()}
      onchange={(e) => set(e.currentTarget.value)}
      class="w-44 rounded-lg border border-border bg-surface-2 px-2 py-1 text-sm text-text outline-none focus:border-accent"
    >
      {#each choices as ch (ch.value)}
        <option value={ch.value}>{ch.label}</option>
      {/each}
    </select>
  </label>
{/snippet}
