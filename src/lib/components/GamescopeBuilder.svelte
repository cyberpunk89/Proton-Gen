<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import SelectField from "./SelectField.svelte";
  import { Monitor as MonitorIcon } from "phosphor-svelte";
  import {
    FILTERS,
    GAME_RESOLUTIONS,
    SCALERS,
    buildGamescope,
    emptyGamescope,
    gamescopeWarnings,
    parseGamescope,
    type GamescopeConfig,
  } from "$lib/gamescope";

  let { onapply }: { onapply?: () => void } = $props();

  // Seeded from the wrapper's current arguments each time the dialog opens
  // (it stays mounted between opens — see OverlayBuilders.svelte), so it
  // edits what's there rather than starting over.
  let c = $state<GamescopeConfig>(emptyGamescope());
  $effect(() => {
    if (!app.gamescopeBuilderOpen) return;
    const w = app.wrap["gamescope"];
    c = parseGamescope(w?.enabled ? w.value : (w?.value ?? "-f"));
  });

  let args = $derived(buildGamescope(c));
  let warnings = $derived(
    gamescopeWarnings(c, { mangohudWrapper: app.wrap["mangohud"]?.enabled ?? false }),
  );

  function useMonitor(m: { width: number; height: number; refresh_hz: number | null }) {
    c.outW = String(m.width);
    c.outH = String(m.height);
    if (m.refresh_hz) c.refresh = String(m.refresh_hz);
    c.fullscreen = true;
  }

  function useGameRes(w: number, h: number) {
    c.gameW = String(w);
    c.gameH = String(h);
    // Rendering below the output is the point; pick an upscaler if none is set.
    if (!c.filter && c.outW && Number(c.outW) > w) c.filter = "fsr";
  }

  function apply() {
    app.applyGamescope(args);
    toast.success("gamescope configured", {
      action: { label: "Undo", onClick: () => app.undo() },
    });
    onapply?.();
  }

  const isActive = (w: string, h: string, pw: number, ph: number) =>
    w === String(pw) && h === String(ph);
</script>

<div class="space-y-4">
  <p class="text-xs text-muted">
    gamescope runs the game in its own compositor: render at one size, show at another,
    upscale between them, and cap or sync the frame rate. Flags this builder doesn't model are
    kept as they are.
  </p>

  <div class="grid gap-5 md:grid-cols-2">
    <div class="space-y-4">
      <!-- Output: the screen gamescope draws to -->
      <section class="space-y-2">
        <h3 class="text-[11px] font-medium uppercase tracking-wider text-muted">Output (your screen)</h3>
        {#if app.hardware.monitors.length}
          <div class="flex flex-wrap gap-1.5">
            {#each app.hardware.monitors as m (m.connector)}
              <button
                onclick={() => useMonitor(m)}
                aria-pressed={isActive(c.outW, c.outH, m.width, m.height)}
                class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] transition {isActive(
                  c.outW,
                  c.outH,
                  m.width,
                  m.height,
                )
                  ? 'border-accent bg-accent/15 text-text'
                  : 'border-border text-subtext hover:border-accent/50'}"
                title="Fill {m.connector} at its native mode"
              >
                <MonitorIcon size={12} />
                {m.connector} · {m.width}×{m.height}{m.refresh_hz ? ` @ ${m.refresh_hz} Hz` : ""}
              </button>
            {/each}
          </div>
        {/if}
        <div class="flex items-center gap-2">
          {@render num("Output width", () => c.outW, (v) => (c.outW = v), "2560")}
          <span class="text-muted">×</span>
          {@render num("Output height", () => c.outH, (v) => (c.outH = v), "1440")}
          <span class="ml-1 text-xs text-muted">@</span>
          {@render num("Refresh rate", () => c.refresh, (v) => (c.refresh = v), "Hz")}
        </div>
      </section>

      <!-- Game: the size the game renders at -->
      <section class="space-y-2 border-t border-border/60 pt-3">
        <h3 class="text-[11px] font-medium uppercase tracking-wider text-muted">Game renders at</h3>
        <div class="flex flex-wrap gap-1.5">
          {#each GAME_RESOLUTIONS as r (r.label)}
            <button
              onclick={() => useGameRes(r.w, r.h)}
              aria-pressed={isActive(c.gameW, c.gameH, r.w, r.h)}
              class="rounded-full border px-2.5 py-1 text-[11px] transition {isActive(c.gameW, c.gameH, r.w, r.h)
                ? 'border-accent bg-accent/15 text-text'
                : 'border-border text-subtext hover:border-accent/50'}">{r.label}</button
            >
          {/each}
          <button
            onclick={() => ((c.gameW = ""), (c.gameH = ""))}
            class="rounded-full border border-border px-2.5 py-1 text-[11px] text-subtext transition hover:border-accent/50"
            >Same as output</button
          >
        </div>
        <div class="flex items-center gap-2">
          {@render num("Game width", () => c.gameW, (v) => (c.gameW = v), "1920")}
          <span class="text-muted">×</span>
          {@render num("Game height", () => c.gameH, (v) => (c.gameH = v), "1080")}
        </div>
      </section>

      <!-- Upscaling -->
      <section class="space-y-2 border-t border-border/60 pt-3">
        <h3 class="text-[11px] font-medium uppercase tracking-wider text-muted">Upscaling</h3>
        {@render pick("Filter", FILTERS, () => c.filter, (v) => (c.filter = v))}
        {#if c.filter === "fsr" || c.filter === "nis"}
          <label class="flex items-center justify-between gap-2">
            <span class="text-sm text-subtext">Sharpness <span class="text-xs text-muted">(0 = sharpest)</span></span>
            <input
              type="range"
              min="0"
              max="20"
              step="1"
              value={c.sharpness || "2"}
              oninput={(e) => (c.sharpness = e.currentTarget.value)}
              aria-label="Upscale sharpness"
              class="w-28 accent-[var(--accent)]"
            />
            <span class="w-6 text-right font-mono text-xs text-muted">{c.sharpness || "2"}</span>
          </label>
        {/if}
        {@render pick("Scaler", SCALERS, () => c.scaler, (v) => (c.scaler = v))}
      </section>
    </div>

    <div class="space-y-4">
      <section class="space-y-2">
        <h3 class="text-[11px] font-medium uppercase tracking-wider text-muted">Window & display</h3>
        {@render toggle("Fullscreen", "-f", () => c.fullscreen, (v) => (c.fullscreen = v))}
        {@render toggle("Borderless", "-b", () => c.borderless, (v) => (c.borderless = v))}
        {@render toggle("HDR", "--hdr-enabled", () => c.hdr, (v) => (c.hdr = v))}
        {@render toggle("Adaptive sync (VRR)", "--adaptive-sync", () => c.adaptiveSync, (v) => (c.adaptiveSync = v))}
        {@render toggle("Grab the cursor", "--force-grab-cursor", () => c.grabCursor, (v) => (c.grabCursor = v))}
      </section>

      <section class="space-y-2 border-t border-border/60 pt-3">
        <h3 class="text-[11px] font-medium uppercase tracking-wider text-muted">Frame rate & extras</h3>
        <div class="flex items-center justify-between gap-2">
          <span class="text-sm text-subtext">Frame-rate limit</span>
          {@render num("Frame-rate limit", () => c.fpsLimit, (v) => (c.fpsLimit = v), "off")}
        </div>
        {@render toggle("MangoHud via gamescope", "--mangoapp", () => c.mangoapp, (v) => (c.mangoapp = v))}
        {@render toggle("Steam integration", "-e", () => c.steam, (v) => (c.steam = v))}
        {#if c.passthrough.length}
          <p class="text-[11px] leading-snug text-muted">
            Kept as-is: <span class="font-mono">{c.passthrough.join(" ")}</span>
          </p>
        {/if}
      </section>

      <section class="space-y-1.5 border-t border-border/60 pt-3">
        <h3 class="text-[11px] font-medium uppercase tracking-wider text-muted">Result</h3>
        <p class="break-all rounded-lg bg-surface-2/60 px-2.5 py-2 font-mono text-xs text-subtext">
          gamescope {args || "-f"} --
        </p>
        {#each warnings as w (w)}
          <p class="text-[11px] leading-snug text-yellow">{w}</p>
        {/each}
      </section>
    </div>
  </div>

  <div class="sticky bottom-0 -mx-1 flex justify-end bg-surface-solid px-1 pb-1 pt-2">
    <button
      onclick={apply}
      class="rounded-lg px-3 py-1.5 text-sm font-medium transition active:scale-95"
      style="background: var(--accent); color: var(--on-accent)">Apply to command</button
    >
  </div>
</div>

{#snippet num(label: string, get: () => string, set: (v: string) => void, placeholder: string)}
  <input
    type="text"
    inputmode="numeric"
    value={get()}
    oninput={(e) => set(e.currentTarget.value.replace(/[^0-9]/g, ""))}
    aria-label={label}
    {placeholder}
    class="w-16 rounded-lg border border-border bg-surface-2 px-2 py-1 font-mono text-xs text-text outline-none placeholder:text-muted focus:border-accent"
  />
{/snippet}

{#snippet toggle(label: string, flag: string, get: () => boolean, set: (v: boolean) => void)}
  <label class="flex items-center gap-2">
    <input
      type="checkbox"
      checked={get()}
      onchange={(e) => set(e.currentTarget.checked)}
      class="accent-[var(--accent)]"
    />
    <span class="text-sm text-subtext">{label}</span>
    <span class="ml-auto font-mono text-[11px] text-muted">{flag}</span>
  </label>
{/snippet}

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
