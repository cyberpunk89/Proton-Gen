<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";

  const METRICS: { key: string; token: string; label: string }[] = [
    { key: "fps", token: "fps", label: "FPS" },
    { key: "frametime", token: "frame_timing", label: "Frame timing" },
    { key: "cpu_load", token: "cpu_stats", label: "CPU load" },
    { key: "gpu_load", token: "gpu_stats", label: "GPU load" },
    { key: "cpu_temp", token: "cpu_temp", label: "CPU temp" },
    { key: "gpu_temp", token: "gpu_temp", label: "GPU temp" },
    { key: "ram", token: "ram", label: "RAM" },
    { key: "vram", token: "vram", label: "VRAM" },
    { key: "gpu_name", token: "gpu_name", label: "GPU name" },
  ];

  let fpsLimit = $state("");

  // Seed the picker from the current MANGOHUD_CONFIG so a restored/applied
  // overlay is reflected here; fall back to a sensible default when empty.
  function seedChecks(): Record<string, boolean> {
    const raw = app.env["MANGOHUD_CONFIG"]?.value ?? "";
    const seeded: Record<string, boolean> = {};
    for (const t of raw.split(",").map((s) => s.trim()).filter(Boolean)) {
      const m = METRICS.find((x) => x.token === t);
      if (m) seeded[m.key] = true;
      const fl = /^fps_limit=(\d+)$/.exec(t);
      if (fl && Number(fl[1]) > 0) fpsLimit = fl[1];
    }
    return Object.keys(seeded).length ? seeded : { fps: true, frametime: true };
  }

  let checks = $state<Record<string, boolean>>(seedChecks());

  let config = $derived.by(() => {
    const parts = METRICS.filter((m) => checks[m.key]).map((m) => m.token);
    const n = parseInt(fpsLimit.trim(), 10);
    if (Number.isFinite(n) && n > 0) parts.push(`fps_limit=${n}`);
    return parts.join(",");
  });

  function apply() {
    app.applyMango(config);
    toast.show("MangoHud overlay applied");
  }
</script>

<div class="space-y-3">
  <p class="text-xs text-muted">Pick what the overlay shows, then apply it to the command.</p>

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

  <label class="flex items-center gap-2">
    <span class="text-sm text-subtext">FPS limit</span>
    <input
      bind:value={fpsLimit}
      placeholder="0"
      class="w-20 rounded-lg border border-border bg-surface-2 px-2 py-1 text-sm text-text outline-none focus:border-accent"
    />
  </label>

  <p class="overflow-x-auto rounded-lg bg-mantle p-2 font-mono text-xs text-muted">
    MANGOHUD_CONFIG={config || "none"}
  </p>

  <div class="flex justify-end">
    <button
      onclick={apply}
      class="rounded-lg px-3 py-1.5 text-sm font-medium transition active:scale-95"
      style="background: var(--accent); color: var(--on-accent)">Apply overlay</button
    >
  </div>
</div>
