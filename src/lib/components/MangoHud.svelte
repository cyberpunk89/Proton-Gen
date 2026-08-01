<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import {
    METRICS,
    POSITIONS,
    COLOR_DEFS,
    DEFAULT_COLORS,
    parseConfig,
    buildConfig,
    hexToRgba,
    type OverlayConfig,
  } from "$lib/mangohud";
  import { open } from "@tauri-apps/plugin-dialog";
  import { untrack } from "svelte";
  import { FolderOpen } from "phosphor-svelte";

  let { onapply }: { onapply?: () => void } = $props();

  // The file dialog only works inside the Tauri shell; in `pnpm dev` (plain
  // browser) it throws, so we hide the Browse button there.
  const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

  // The 13 controls stay local $state rather than writing straight through to
  // the store: the user must be able to build a config and abandon it.
  const seed = parseConfig(app.env["MANGOHUD_CONFIG"]?.value ?? "");

  let checks = $state<Record<string, boolean>>(seed.checks);
  let fpsLimit = $state(seed.fpsLimit);
  let position = $state(seed.position);
  let fontSize = $state(seed.fontSize);
  let roundCorners = $state(seed.roundCorners);
  let horizontal = $state(seed.horizontal);
  let compact = $state(seed.compact);
  let bgAlphaOn = $state(seed.bgAlphaOn);
  let bgAlpha = $state(seed.bgAlpha);
  let alphaOn = $state(seed.alphaOn);
  let alpha = $state(seed.alpha);
  let colorOn = $state<Record<string, boolean>>(seed.colorOn);
  let colorVal = $state<Record<string, string>>(seed.colorVal);

  function reseed(s: OverlayConfig) {
    checks = s.checks;
    fpsLimit = s.fpsLimit;
    position = s.position;
    fontSize = s.fontSize;
    roundCorners = s.roundCorners;
    horizontal = s.horizontal;
    compact = s.compact;
    bgAlphaOn = s.bgAlphaOn;
    bgAlpha = s.bgAlpha;
    alphaOn = s.alphaOn;
    alpha = s.alpha;
    colorOn = s.colorOn;
    colorVal = s.colorVal;
  }

  // Preset-file mode — seed from an already-applied MANGOHUD_CONFIGFILE.
  const fileEnv = app.env["MANGOHUD_CONFIGFILE"];
  let filePath = $state(fileEnv?.value ?? "");
  let mode = $state<"build" | "file">(fileEnv?.enabled && fileEnv.value ? "file" : "build");

  let config = $derived(
    buildConfig({
      checks,
      fpsLimit,
      position,
      fontSize,
      roundCorners,
      horizontal,
      compact,
      bgAlphaOn,
      bgAlpha,
      alphaOn,
      alpha,
      colorOn,
      colorVal,
    }),
  );

  // --------------------------- follow the store -----------------------------

  /**
   * Re-seed when the env changes underneath us — an undo, a recipe, a preset,
   * or a hand edit on the MANGOHUD_CONFIG row while this panel is open. The
   * seed above only covers construction, which merely *looked* right because
   * the old home unmounted the component between visits.
   *
   * The self-echo is guarded by the value comparison, not by a flag: after
   * `apply()` the env equals `config`, so there is nothing to re-seed. `config`
   * is read untracked on purpose — tracking it would re-run this effect on
   * every keystroke and stomp the edit that caused it.
   */
  let sourceConfig = $derived(app.env["MANGOHUD_CONFIG"]?.value ?? "");
  $effect(() => {
    const src = sourceConfig;
    if (src === untrack(() => config)) return;
    reseed(parseConfig(src));
  });

  let sourceFile = $derived(app.env["MANGOHUD_CONFIGFILE"]?.value ?? "");
  let sourceFileOn = $derived(app.env["MANGOHUD_CONFIGFILE"]?.enabled === true);
  $effect(() => {
    const path = sourceFile;
    const on = sourceFileOn;
    untrack(() => {
      if (path !== filePath) filePath = path;
      // Follow the env into (and back out of) file mode, so an undo of "use
      // config file" doesn't leave the panel showing a tab that is no longer
      // what the command does.
      if (on && path) mode = "file";
      else if (!on && mode === "file") mode = "build";
    });
  });

  // ------------------------------ live preview ------------------------------

  function col(key: string): string {
    return colorOn[key] ? colorVal[key] : DEFAULT_COLORS[key];
  }

  // Sample rows shown in the mock overlay, in MangoHud's rough top-to-bottom order.
  const PREVIEW_ROWS: { key: string; label: string; value: string; kind: string }[] = [
    { key: "gpu_load", label: "GPU", value: "47%", kind: "gpu" },
    { key: "gpu_temp", label: "GPU", value: "49°C", kind: "gpu" },
    { key: "cpu_load", label: "CPU", value: "32%", kind: "cpu" },
    { key: "cpu_temp", label: "CPU", value: "51°C", kind: "cpu" },
    { key: "ram", label: "RAM", value: "11.2 GiB", kind: "text" },
    { key: "vram", label: "VRAM", value: "6.4 GiB", kind: "text" },
    { key: "gpu_name", label: "", value: "NVIDIA RTX 4070", kind: "gpu" },
    { key: "fps", label: "", value: "60 FPS", kind: "text" },
    { key: "frametime", label: "", value: "6.9 ms", kind: "text" },
  ];

  let pv = $derived.by(() => {
    const bgA = bgAlphaOn ? Number(bgAlpha) : 0.5;
    return {
      fs: Math.min(Math.max(parseFloat(fontSize) || 24, 8), 34), // clamped for the preview only
      text: col("text"),
      bg: hexToRgba(col("background"), Number.isFinite(bgA) ? bgA : 0.5),
      radius: Math.max(0, parseFloat(roundCorners) || 0),
      opacity: alphaOn ? Number(alpha) || 1 : 1,
      align: position.startsWith("bottom") ? "flex-end" : "flex-start",
      justify: position.endsWith("center")
        ? "center"
        : position.endsWith("right")
          ? "flex-end"
          : "flex-start",
    };
  });

  let previewRows = $derived(
    PREVIEW_ROWS.filter((r) => checks[r.key]).map((r) => ({
      ...r,
      color: r.kind === "gpu" ? col("gpu") : r.kind === "cpu" ? col("cpu") : pv.text,
    })),
  );

  async function browse() {
    try {
      const sel = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "MangoHud config", extensions: ["conf"] }],
      });
      if (typeof sel === "string") filePath = sel;
    } catch (e) {
      console.error("file dialog failed", e);
    }
  }

  function apply() {
    if (mode === "file") {
      const p = filePath.trim();
      if (!p) {
        toast.error("Enter a config file path first");
        return;
      }
      app.applyMangoFile(p);
      toast.success("MangoHud config file applied");
    } else {
      app.applyMango(config);
      toast.success("MangoHud overlay applied");
    }
    onapply?.();
  }
</script>

<div class="space-y-3">
  <!-- Mode toggle -->
  <div class="flex gap-1 rounded-lg bg-mantle p-0.5 text-xs">
    <button
      onclick={() => (mode = "build")}
      class="flex-1 rounded-md px-2 py-1 font-medium transition"
      class:bg-surface-2={mode === "build"}
      class:text-text={mode === "build"}
      class:text-muted={mode !== "build"}>Build overlay</button
    >
    <button
      onclick={() => (mode = "file")}
      class="flex-1 rounded-md px-2 py-1 font-medium transition"
      class:bg-surface-2={mode === "file"}
      class:text-text={mode === "file"}
      class:text-muted={mode !== "file"}>Use config file</button
    >
  </div>

  {#if mode === "build"}
    <p class="text-xs text-muted">Pick what the overlay shows and how it looks, then apply it.</p>

    <!--
      Two columns at the dialog's width: the controls scroll on the left while
      the preview and the resulting string stay put on the right, so you can see
      what a slider does without hunting for the overlay below the fold.
    -->
    <div class="grid gap-5 md:grid-cols-2">
      <div class="space-y-3">
        <!-- Metrics -->
        <div class="grid grid-cols-2 gap-1.5">
          {#each METRICS as m (m.key)}
            <label
              class="flex cursor-pointer items-center gap-2 rounded-lg px-2 py-1.5 hover:bg-surface-2"
            >
              <input type="checkbox" bind:checked={checks[m.key]} class="accent-[var(--accent)]" />
              <span class="text-sm text-subtext">{m.label}</span>
            </label>
          {/each}
        </div>

        <!-- Appearance & layout -->
        <div class="space-y-2 border-t border-border/60 pt-3">
          <p class="text-[11px] font-medium uppercase tracking-wider text-muted">Appearance</p>

          <label class="flex items-center justify-between gap-2">
            <span class="text-sm text-subtext">Position</span>
            <select
              bind:value={position}
              class="w-40 rounded-lg border border-border bg-surface-2 px-2 py-1 text-sm text-text outline-none focus:border-accent"
            >
              {#each POSITIONS as p (p.value)}
                <option value={p.value}>{p.label}</option>
              {/each}
            </select>
          </label>

          <label class="flex items-center justify-between gap-2">
            <span class="text-sm text-subtext">Font size</span>
            <input
              bind:value={fontSize}
              placeholder="24"
              class="w-20 rounded-lg border border-border bg-surface-2 px-2 py-1 text-sm text-text outline-none focus:border-accent"
            />
          </label>

          <label class="flex items-center justify-between gap-2">
            <span class="text-sm text-subtext">Round corners</span>
            <input
              bind:value={roundCorners}
              placeholder="0"
              class="w-20 rounded-lg border border-border bg-surface-2 px-2 py-1 text-sm text-text outline-none focus:border-accent"
            />
          </label>

          <label class="flex items-center gap-2">
            <input type="checkbox" bind:checked={bgAlphaOn} class="accent-[var(--accent)]" />
            <span class="text-sm text-subtext">Background opacity</span>
            <input
              type="range"
              min="0"
              max="1"
              step="0.05"
              bind:value={bgAlpha}
              disabled={!bgAlphaOn}
              aria-label="Background opacity value"
              class="ml-auto w-28 accent-[var(--accent)] disabled:opacity-40"
            />
            <span class="w-8 text-right font-mono text-xs text-muted">{bgAlpha}</span>
          </label>

          <label class="flex items-center gap-2">
            <input type="checkbox" bind:checked={alphaOn} class="accent-[var(--accent)]" />
            <span class="text-sm text-subtext">Overlay opacity</span>
            <input
              type="range"
              min="0"
              max="1"
              step="0.05"
              bind:value={alpha}
              disabled={!alphaOn}
              aria-label="Overlay opacity value"
              class="ml-auto w-28 accent-[var(--accent)] disabled:opacity-40"
            />
            <span class="w-8 text-right font-mono text-xs text-muted">{alpha}</span>
          </label>

          <div class="flex gap-4">
            <label class="flex cursor-pointer items-center gap-2">
              <input type="checkbox" bind:checked={horizontal} class="accent-[var(--accent)]" />
              <span class="text-sm text-subtext">Horizontal</span>
            </label>
            <label class="flex cursor-pointer items-center gap-2">
              <input type="checkbox" bind:checked={compact} class="accent-[var(--accent)]" />
              <span class="text-sm text-subtext">Compact</span>
            </label>
          </div>
        </div>

        <!-- Colors -->
        <div class="space-y-2 border-t border-border/60 pt-3">
          <p class="text-[11px] font-medium uppercase tracking-wider text-muted">Colors</p>
          <div class="grid grid-cols-2 gap-1.5">
            {#each COLOR_DEFS as c (c.key)}
              <label class="flex items-center gap-2 rounded-lg px-2 py-1.5 hover:bg-surface-2">
                <input type="checkbox" bind:checked={colorOn[c.key]} class="accent-[var(--accent)]" />
                <span class="flex-1 text-sm text-subtext">{c.label}</span>
                <input
                  type="color"
                  bind:value={colorVal[c.key]}
                  disabled={!colorOn[c.key]}
                  aria-label="{c.label} colour"
                  class="h-6 w-8 cursor-pointer rounded border border-border bg-surface-2 disabled:opacity-40"
                />
              </label>
            {/each}
          </div>
        </div>

        <!-- FPS limit -->
        <label class="flex items-center gap-2 border-t border-border/60 pt-3">
          <span class="text-sm text-subtext">FPS limit</span>
          <input
            bind:value={fpsLimit}
            placeholder="0"
            class="w-20 rounded-lg border border-border bg-surface-2 px-2 py-1 text-sm text-text outline-none focus:border-accent"
          />
        </label>
      </div>

      <!-- Live preview -->
      <div class="space-y-1.5 md:sticky md:top-0 md:self-start">
          <p class="text-[11px] font-medium uppercase tracking-wider text-muted">Preview</p>
          <div
            class="relative overflow-hidden rounded-lg"
            style="height: 200px; background: linear-gradient(135deg, #223047 0%, #0e1626 55%, #060a12 100%);"
          >
            <div
              class="flex h-full w-full p-3"
              style="align-items: {pv.align}; justify-content: {pv.justify};"
            >
              <div
                style="
                  background: {pv.bg};
                  color: {pv.text};
                  border-radius: {pv.radius}px;
                  opacity: {pv.opacity};
                  font-size: {pv.fs}px;
                  font-family: monospace;
                  line-height: 1.25;
                  padding: {compact ? '0.15em 0.5em' : '0.4em 0.7em'};
                  display: flex;
                  flex-direction: {horizontal ? 'row' : 'column'};
                  gap: {horizontal ? '0.9em' : compact ? '0' : '0.15em'};
                "
              >
                {#each previewRows as r (r.key)}
                  <div
                    style="display: flex; white-space: nowrap; gap: 0.6em; {horizontal
                      ? 'flex-direction: column; align-items: center;'
                      : 'justify-content: space-between;'}"
                  >
                    {#if r.label}<span style="color: {r.color}; font-weight: 600;">{r.label}</span>{/if}
                    <span
                      style="color: {r.key === 'gpu_name' ? r.color : pv.text}; font-weight: {r.key ===
                      'fps'
                        ? '700'
                        : '400'};">{r.value}</span
                    >
                  </div>
                {/each}
                {#if !previewRows.length}
                  <span style="opacity: 0.6; font-size: 0.7em;">enable a metric…</span>
                {/if}
              </div>
            </div>
          </div>

        <p class="overflow-x-auto rounded-lg bg-mantle p-2 font-mono text-xs text-muted">
          MANGOHUD_CONFIG={config || "none"}
        </p>
      </div>
    </div>
  {:else}
    <p class="text-xs text-muted">
      Point MangoHud at an existing config file. Whatever you built on the “Build overlay” tab is
      disabled when a file is applied, since <span class="font-mono">MANGOHUD_CONFIG</span> would otherwise
      override it.
    </p>

    <div class="flex gap-2">
      <input
        bind:value={filePath}
        placeholder="~/.config/MangoHud/MangoHud.conf"
        class="min-w-0 flex-1 rounded-lg border border-border bg-surface-2 px-2 py-1.5 text-sm text-text outline-none focus:border-accent"
      />
      {#if isTauri}
        <button
          onclick={browse}
          class="flex items-center gap-1.5 rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-sm text-subtext transition hover:bg-surface-2/70 active:scale-95"
        >
          <FolderOpen size={15} /> Browse
        </button>
      {/if}
    </div>

    <p class="overflow-x-auto rounded-lg bg-mantle p-2 font-mono text-xs text-muted">
      MANGOHUD_CONFIGFILE={filePath.trim() || "none"}
    </p>
  {/if}

  <!-- Sticky: the panel is taller than the dialog's scroll area on a short
       screen, and the primary action should never be the thing you have to go
       looking for. -->
  <div class="sticky bottom-0 -mx-1 flex justify-end bg-surface-solid px-1 pb-1 pt-2">
    <button
      onclick={apply}
      class="rounded-lg px-3 py-1.5 text-sm font-medium transition active:scale-95"
      style="background: var(--accent); color: var(--on-accent)"
      >{mode === "file" ? "Apply config file" : "Apply overlay"}</button
    >
  </div>
</div>
