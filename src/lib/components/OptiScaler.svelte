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
  import { untrack } from "svelte";

  let { onapply }: { onapply?: () => void } = $props();

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
