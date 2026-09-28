<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { ipc } from "$lib/ipc";
  import { toast } from "$lib/toast.svelte";
  import SelectField from "./SelectField.svelte";
  import {
    TOGGLE_KEYS,
    SMAA_EDGE_DETECTION,
    emptyVkBasalt,
    parseVkBasalt,
    buildVkBasalt,
    type VkBasaltConfig,
  } from "$lib/vkbasalt";

  let { onapply }: { onapply?: () => void } = $props();

  // Unlike MangoHud/OptiScaler, vkBasalt has no live env value to seed from —
  // its only config surface is the real file. So the seed is fetched instead
  // of derived, and re-fetched every time this dialog opens (it stays mounted
  // across opens/closes, same as MangoHud/OptiScaler — see OverlayBuilders.svelte),
  // in case the file changed since the last time it was open.
  let c = $state<VkBasaltConfig>(emptyVkBasalt());
  let loaded = $state(false);

  $effect(() => {
    if (!app.vkBuilderOpen) return;
    loaded = false;
    ipc
      .vkbasaltReadConfig()
      .then((text) => {
        c = parseVkBasalt(text);
        loaded = true;
      })
      .catch((e) => {
        toast.error(`Couldn't read vkBasalt.conf: ${e}`, { ms: 6000 });
        loaded = true;
      });
  });

  let config = $derived(buildVkBasalt(c));

  function apply() {
    app.pendingVkBasaltSystemConfig = config;
    onapply?.();
    app.vkSystemConfirmOpen = true;
  }
</script>

<div class="space-y-3">
  <p class="text-xs text-muted">
    Compose vkBasalt.conf settings, then set them as the real, system-wide config every
    vkBasalt-enabled program reads. Only the options you change here are written; the rest
    stay at vkBasalt's defaults.
  </p>

  {#if !loaded}
    <p class="text-xs text-muted">Reading the current vkBasalt.conf…</p>
  {:else}
    <div class="grid gap-5 md:grid-cols-2">
      <div class="space-y-3">
        <!-- CAS sharpening -->
        <div class="space-y-2">
          <label class="flex items-center gap-2">
            <input type="checkbox" bind:checked={c.casOn} class="accent-[var(--accent)]" />
            <span class="text-sm text-subtext">CAS sharpening</span>
            <input
              type="range"
              min="-1"
              max="1"
              step="0.05"
              bind:value={c.casSharpness}
              disabled={!c.casOn}
              aria-label="CAS sharpness"
              class="ml-auto w-28 accent-[var(--accent)] disabled:opacity-40"
            />
            <span class="w-10 text-right font-mono text-xs text-muted">{c.casSharpness}</span>
          </label>
        </div>

        <!-- DLS (denoised luma sharpening) -->
        <div class="space-y-2 border-t border-border/60 pt-3">
          <label class="flex items-center gap-2">
            <input type="checkbox" bind:checked={c.dlsOn} class="accent-[var(--accent)]" />
            <span class="text-sm text-subtext">DLS (denoised luma sharpening)</span>
          </label>
          {#if c.dlsOn}
            <label class="flex items-center justify-between gap-2 pl-6">
              <span class="text-sm text-subtext">Sharpness</span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                bind:value={c.dlsSharpness}
                aria-label="DLS sharpness"
                class="w-28 accent-[var(--accent)]"
              />
              <span class="w-10 text-right font-mono text-xs text-muted">{c.dlsSharpness}</span>
            </label>
            <label class="flex items-center justify-between gap-2 pl-6">
              <span class="text-sm text-subtext">Denoise</span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.01"
                bind:value={c.dlsDenoise}
                aria-label="DLS denoise"
                class="w-28 accent-[var(--accent)]"
              />
              <span class="w-10 text-right font-mono text-xs text-muted">{c.dlsDenoise}</span>
            </label>
          {/if}
        </div>

        <!-- FXAA -->
        <div class="space-y-2 border-t border-border/60 pt-3">
          <label class="flex items-center gap-2">
            <input type="checkbox" bind:checked={c.fxaaOn} class="accent-[var(--accent)]" />
            <span class="text-sm text-subtext">FXAA</span>
          </label>
          {#if c.fxaaOn}
            <label class="flex items-center justify-between gap-2 pl-6">
              <span class="text-sm text-subtext">Subpixel quality</span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                bind:value={c.fxaaQualitySubpix}
                aria-label="FXAA subpixel quality"
                class="w-28 accent-[var(--accent)]"
              />
              <span class="w-10 text-right font-mono text-xs text-muted"
                >{c.fxaaQualitySubpix}</span
              >
            </label>
            <label class="flex items-center justify-between gap-2 pl-6">
              <span class="text-sm text-subtext">Edge threshold</span>
              <input
                type="range"
                min="0.063"
                max="0.333"
                step="0.001"
                bind:value={c.fxaaQualityEdgeThreshold}
                aria-label="FXAA edge threshold"
                class="w-28 accent-[var(--accent)]"
              />
              <span class="w-10 text-right font-mono text-xs text-muted"
                >{c.fxaaQualityEdgeThreshold}</span
              >
            </label>
          {/if}
        </div>

        <!-- SMAA -->
        <div class="space-y-2 border-t border-border/60 pt-3">
          <label class="flex items-center gap-2">
            <input type="checkbox" bind:checked={c.smaaOn} class="accent-[var(--accent)]" />
            <span class="text-sm text-subtext">SMAA</span>
          </label>
          {#if c.smaaOn}
            {@render pick(
              "Edge detection",
              SMAA_EDGE_DETECTION,
              () => c.smaaEdgeDetection,
              (v) => (c.smaaEdgeDetection = v),
            )}
            <label class="flex items-center justify-between gap-2 pl-6">
              <span class="text-sm text-subtext">Threshold</span>
              <input
                type="range"
                min="0"
                max="0.5"
                step="0.01"
                bind:value={c.smaaThreshold}
                aria-label="SMAA threshold"
                class="w-28 accent-[var(--accent)]"
              />
              <span class="w-10 text-right font-mono text-xs text-muted">{c.smaaThreshold}</span>
            </label>
          {/if}
        </div>
      </div>

      <div class="space-y-4">
        <!-- Global -->
        <div class="space-y-2">
          <p class="text-[11px] font-medium uppercase tracking-wider text-muted">Global</p>
          {@render pick(
            "In-game toggle key",
            TOGGLE_KEYS,
            () => c.toggleKey,
            (v) => (c.toggleKey = v),
          )}
          <label class="flex items-center gap-2">
            <input type="checkbox" bind:checked={c.enableOnLaunch} class="accent-[var(--accent)]" />
            <span class="text-sm text-subtext">Enable effects on launch</span>
          </label>
        </div>

        <!-- Resulting string -->
        <div class="space-y-1.5 border-t border-border/60 pt-3">
          <p class="text-[11px] font-medium uppercase tracking-wider text-muted">Result</p>
          <p class="overflow-x-auto rounded-lg bg-mantle p-2 font-mono text-xs break-all whitespace-pre-wrap text-muted">
            {config || "none"}
          </p>
          {#if c.passthrough.length || c.effectsPassthrough.length}
            <p class="text-[11px] leading-snug text-muted">
              {c.passthrough.length + c.effectsPassthrough.length} setting{c.passthrough.length +
                c.effectsPassthrough.length ===
              1
                ? ""
                : "s"} this builder doesn't model {c.passthrough.length +
                c.effectsPassthrough.length ===
              1
                ? "is"
                : "are"} kept as-is.
            </p>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  <div class="sticky bottom-0 -mx-1 flex justify-end bg-surface-solid px-1 pb-1 pt-2">
    <button
      onclick={apply}
      disabled={!loaded}
      class="rounded-lg px-3 py-1.5 text-sm font-medium transition active:scale-95 disabled:opacity-40"
      style="background: var(--accent); color: var(--on-accent)">Set as vkBasalt config…</button
    >
  </div>
</div>

{#snippet pick(
  label: string,
  choices: { value: string; label: string }[],
  get: () => string,
  set: (v: string) => void,
)}
  <div class="flex items-center justify-between gap-2">
    <span class="text-sm text-subtext">{label}</span>
    <SelectField {label} value={get()} options={choices} onValueChange={set} width="w-44" />
  </div>
{/snippet}
