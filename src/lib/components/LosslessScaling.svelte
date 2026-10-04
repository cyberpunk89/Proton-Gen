<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import SelectField from "./SelectField.svelte";
  import {
    MULTIPLIERS,
    activeInMatches,
    buildLsfg,
    envDllPath,
    lsfgGenerates,
    readLsfg,
    LSFG_BUILDER_KEYS,
    type LsfgMode,
    type LsfgSettings,
  } from "$lib/lsfg";
  import { formatExtraEnv } from "$lib/shell";
  import { copyText, openSteamUrl, steamLibraryUrl } from "$lib/util";
  import { untrack, type Component } from "svelte";
  import {
    ArrowClockwise,
    CheckCircle,
    Copy,
    MagicWand,
    Prohibit,
    SlidersHorizontal,
    Sparkle,
    Stack,
    WarningCircle,
    XCircle,
  } from "phosphor-svelte";

  let { onapply }: { onapply?: () => void } = $props();

  const LOSSLESS_APPID = 993090;

  // Fresh on every open: profiles are edited in lsfg-vk-ui, not here.
  app.refreshLsfgStatus();
  let st = $derived(app.lsfgStatus);
  let profiles = $derived(st?.profiles ?? []);

  /** The launch env as `readLsfg` wants it: an enabled row's value, else null. */
  function envGet(k: string): string | null {
    const row = app.env[k];
    return row?.enabled ? row.value : null;
  }

  // Local $state, applied on demand — the user must be able to explore modes
  // and walk away (same rationale as the MangoHud / OptiScaler builders).
  let s = $state<LsfgSettings>(readLsfg(envGet));

  // Re-seed when the env changes underneath (undo, a recipe, a hand edit on a
  // row) — but not from our own Apply, which writes exactly what `s` holds.
  let source = $derived(JSON.stringify(LSFG_BUILDER_KEYS.map((k) => envGet(k))));
  $effect(() => {
    void source;
    const fresh = untrack(() => readLsfg(envGet));
    if (JSON.stringify(fresh) !== untrack(() => JSON.stringify(s))) s = fresh;
  });

  // Picking "Profile" with nothing chosen yet: preselect the first one.
  $effect(() => {
    if (s.mode === "profile" && !s.profile && profiles.length) {
      untrack(() => (s.profile = profiles[0].name));
    }
  });

  let dllForEnv = $derived(envDllPath(st));
  let pairs = $derived(buildLsfg(s, dllForEnv));
  let preview = $derived(formatExtraEnv(pairs));
  let generates = $derived(lsfgGenerates(s));

  let selectedProfile = $derived(profiles.find((p) => p.name === s.profile));
  let staleProfile = $derived(
    s.mode === "profile" && !!s.profile && !!st?.config_found && !selectedProfile,
  );
  let autoProfiles = $derived(profiles.filter((p) => p.active_in.length));
  let exe = $derived(app.selectedGame?.executable ?? "");
  let autoMatch = $derived(
    exe ? autoProfiles.find((p) => p.active_in.some((a) => activeInMatches(a, exe))) : undefined,
  );

  // Frame pacing helper — lsfg-vk paces to vsync, so the generated rate should
  // land on the display's refresh. Display-only: nothing here is written.
  const REFRESH = [60, 75, 90, 120, 144, 165, 170, 180, 240].map((r) => ({
    value: String(r),
    label: `${r} Hz`,
  }));
  let refresh = $state("144");
  let effMultiplier = $derived(
    s.mode === "custom" ? s.multiplier : (selectedProfile?.multiplier ?? 2),
  );
  let baseCap = $derived(Math.floor(Number(refresh) / effMultiplier));

  const MODES: { id: LsfgMode; title: string; blurb: string; icon: Component }[] = [
    { id: "profile", title: "Profile", blurb: "One of your lsfg-vk profiles", icon: Stack },
    { id: "custom", title: "Per-game", blurb: "Settings just for this game", icon: SlidersHorizontal },
    { id: "auto", title: "Automatic", blurb: "conf.toml decides by game exe", icon: MagicWand },
    { id: "off", title: "Off", blurb: "Never for this game", icon: Prohibit },
  ];

  function profileSummary(p: (typeof profiles)[number]): string {
    const bits = [`${p.multiplier ?? 2}×`, `flow ${Math.round((p.flow_scale ?? 1) * 100)}%`];
    if (p.performance_mode) bits.push("performance");
    return bits.join(" · ");
  }

  async function copyInstall() {
    await copyText("sudo pacman -S lsfg-vk");
    toast.success("Copied: sudo pacman -S lsfg-vk");
  }

  async function openLossless() {
    if (!(await openSteamUrl(steamLibraryUrl(LOSSLESS_APPID)))) {
      toast.error("Couldn't open Steam — search the store for “Lossless Scaling”.");
    }
  }

  function apply() {
    app.applyLsfg(pairs);
    const msg =
      s.mode === "off"
        ? "Lossless Scaling turned off for this game"
        : s.mode === "auto"
          ? "Lossless Scaling left to conf.toml"
          : "Lossless Scaling frame generation applied";
    toast.success(msg);
    onapply?.();
  }
</script>

<div class="space-y-4">
  <!-- Machine status: what has to be true before any of this does anything. -->
  <div class="space-y-1.5 rounded-lg border border-border/60 p-3">
    <div class="flex items-center justify-between gap-2">
      <p class="text-[11px] font-medium uppercase tracking-wider text-muted">This machine</p>
      <button
        onclick={() => app.refreshLsfgStatus()}
        disabled={app.lsfgStatusLoading}
        class="inline-flex items-center gap-1 text-[11px] text-muted transition hover:text-text disabled:opacity-40"
        title="Re-read lsfg-vk's config and look for Lossless.dll again"
      >
        <ArrowClockwise size={11} class={app.lsfgStatusLoading ? "animate-spin" : ""} /> Refresh
      </button>
    </div>
    {#if !st}
      <p class="flex items-center gap-1.5 text-xs text-muted">
        <ArrowClockwise size={12} class="animate-spin" /> Looking for lsfg-vk…
      </p>
    {:else}
      <!-- Layer -->
      <div class="flex items-start gap-2 text-xs">
        {#if st.layer && !st.layer.legacy}
          <CheckCircle size={14} weight="fill" class="mt-px shrink-0 text-green" />
          <span class="min-w-0 text-subtext" title={st.layer.manifest}>
            lsfg-vk {st.layer.version} layer installed
          </span>
        {:else if st.layer}
          <WarningCircle size={14} weight="fill" class="mt-px shrink-0 text-yellow" />
          <span class="min-w-0 text-subtext">
            lsfg-vk 1.x is installed — these settings are for 2.x. Update it
            (<span class="font-mono">sudo pacman -S lsfg-vk</span>).
          </span>
        {:else}
          <XCircle size={14} weight="fill" class="mt-px shrink-0 text-red" />
          <span class="min-w-0 flex-1 text-subtext">
            The lsfg-vk Vulkan layer isn't installed — nothing below takes effect without it.
          </span>
          <button
            onclick={copyInstall}
            class="inline-flex shrink-0 items-center gap-1 rounded border border-border px-1.5 py-0.5 text-[11px] text-subtext hover:border-accent/50 hover:text-text"
          >
            <Copy size={11} /> sudo pacman -S lsfg-vk
          </button>
        {/if}
      </div>

      <!-- Lossless.dll -->
      <div class="flex items-start gap-2 text-xs">
        {#if st.dll}
          <CheckCircle size={14} weight="fill" class="mt-px shrink-0 text-green" />
          <span class="min-w-0 truncate text-subtext" title={st.dll}>
            Lossless Scaling found <span class="font-mono text-muted">{st.dll}</span>
          </span>
        {:else}
          <XCircle size={14} weight="fill" class="mt-px shrink-0 text-red" />
          <span class="min-w-0 flex-1 text-subtext">
            Lossless.dll not found — install Lossless Scaling from Steam (it has to be installed, not
            just owned).
          </span>
          <button
            onclick={openLossless}
            class="shrink-0 rounded border border-border px-1.5 py-0.5 text-[11px] text-subtext hover:border-accent/50 hover:text-text"
          >
            Open in Steam
          </button>
        {/if}
      </div>

      <!-- conf.toml -->
      <div class="flex items-start gap-2 text-xs">
        {#if st.config_error}
          <XCircle size={14} weight="fill" class="mt-px shrink-0 text-red" />
          <span class="min-w-0 text-subtext">
            <span class="font-mono">{st.config_path}</span> doesn't parse: {st.config_error}
          </span>
        {:else if st.config_found}
          <CheckCircle size={14} weight="fill" class="mt-px shrink-0 text-green" />
          <span class="min-w-0 truncate text-subtext" title={st.config_path}>
            {profiles.length} profile{profiles.length === 1 ? "" : "s"} in
            <span class="font-mono text-muted">{st.config_path}</span>
          </span>
        {:else}
          <WarningCircle size={14} weight="fill" class="mt-px shrink-0 text-muted" />
          <span class="min-w-0 text-subtext">
            No lsfg-vk config yet{st.ui_installed ? " — create profiles in lsfg-vk-ui," : ","} or use
            per-game settings below.
          </span>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Mode -->
  <div role="radiogroup" aria-label="How to set up frame generation" class="grid grid-cols-2 gap-2 md:grid-cols-4">
    {#each MODES as m (m.id)}
      {@const on = s.mode === m.id}
      <button
        role="radio"
        aria-checked={on}
        onclick={() => (s.mode = m.id)}
        class="flex flex-col items-start gap-1 rounded-lg border p-2.5 text-left transition {on
          ? 'border-accent bg-accent/10'
          : 'border-border/60 hover:border-accent/40'}"
      >
        <span class="flex items-center gap-1.5 text-sm font-medium {on ? 'text-accent' : 'text-text'}">
          <m.icon size={15} weight={on ? "fill" : "regular"} />
          {m.title}
        </span>
        <span class="text-[11px] leading-snug text-muted">{m.blurb}</span>
      </button>
    {/each}
  </div>

  <!-- Mode body -->
  {#if s.mode === "profile"}
    <div class="space-y-2">
      {#if profiles.length}
        <div role="radiogroup" aria-label="lsfg-vk profile" class="space-y-1">
          {#each profiles as p (p.name)}
            {@const on = s.profile === p.name}
            <button
              role="radio"
              aria-checked={on}
              onclick={() => (s.profile = p.name)}
              class="flex w-full items-center gap-3 rounded-lg border px-3 py-2 text-left transition {on
                ? 'border-accent bg-accent/5'
                : 'border-border/60 hover:border-accent/40'}"
            >
              <span
                class="size-3.5 shrink-0 rounded-full border-2 {on ? 'border-accent bg-accent' : 'border-border'}"
              ></span>
              <span class="min-w-0 flex-1">
                <span class="block truncate text-sm text-text">{p.name}</span>
                {#if p.active_in.length}
                  <span class="block truncate text-[11px] text-muted">
                    Also auto-activates in {p.active_in.join(", ")}
                  </span>
                {/if}
              </span>
              <span class="shrink-0 font-mono text-[11px] text-muted">{profileSummary(p)}</span>
            </button>
          {/each}
        </div>
      {:else}
        <p class="rounded-lg border border-border/60 p-3 text-xs text-muted">
          No profiles to pick from yet. Create some in
          {st?.ui_installed ? "lsfg-vk-ui (your app menu)" : "lsfg-vk's config"} and come back — this list
          refreshes when you return to protongen — or use <button
            class="text-accent hover:underline"
            onclick={() => (s.mode = "custom")}>Per-game</button
          > settings instead.
        </p>
      {/if}
      {#if staleProfile}
        <p class="flex items-start gap-1.5 text-xs text-yellow">
          <WarningCircle size={13} class="mt-px shrink-0" />
          “{s.profile}” isn't in your conf.toml any more — lsfg-vk will fall back to automatic matching.
        </p>
      {/if}
      <p class="text-[11px] leading-snug text-muted">
        Edit profiles in lsfg-vk-ui. Multiplier, flow scale and performance mode reload live — you can
        tune them while the game is running.
      </p>
    </div>
  {:else if s.mode === "custom"}
    <div class="grid gap-4 md:grid-cols-2">
      <div class="space-y-3">
        <div class="space-y-1.5">
          <p class="text-sm text-subtext">Multiplier</p>
          <div role="radiogroup" aria-label="Frame generation multiplier" class="inline-flex rounded-lg border border-border/60 p-0.5">
            {#each MULTIPLIERS.includes(s.multiplier as (typeof MULTIPLIERS)[number]) ? MULTIPLIERS : [...MULTIPLIERS, s.multiplier] as n (n)}
              <button
                role="radio"
                aria-checked={s.multiplier === n}
                onclick={() => (s.multiplier = n)}
                class="rounded-md px-3 py-1 text-sm transition {s.multiplier === n
                  ? 'bg-accent font-medium text-on-accent'
                  : 'text-muted hover:text-text'}">{n}×</button
              >
            {/each}
          </div>
          <p class="text-[11px] leading-snug text-muted">
            2× looks best and adds the least latency. Go higher only with a high-refresh display and a
            base frame rate of 50+ fps.
          </p>
        </div>

        <div class="space-y-1.5">
          <label class="flex items-center gap-2">
            <span class="text-sm text-subtext">Flow scale</span>
            <input
              type="range"
              min="0.25"
              max="1"
              step="0.05"
              bind:value={s.flowScale}
              aria-label="Flow scale"
              class="ml-auto w-36 accent-[var(--accent)]"
            />
            <span class="w-10 text-right font-mono text-xs text-muted">{Math.round(s.flowScale * 100)}%</span>
          </label>
          <p class="text-[11px] leading-snug text-muted">
            Resolution motion is estimated at. Lower is faster: try 50–75% at 1440p/4K.
          </p>
        </div>

        <label class="flex gap-2">
          <input type="checkbox" bind:checked={s.performance} class="mt-0.5 shrink-0 accent-[var(--accent)]" />
          <span class="min-w-0">
            <span class="block text-sm leading-snug text-subtext">Performance mode</span>
            <span class="block text-[11px] leading-snug text-muted">
              Lighter model — for handhelds, iGPUs, or a GPU that's already maxed out.
            </span>
          </span>
        </label>
      </div>

      <div class="space-y-3">
        <details class="group rounded-lg border border-border/60 p-2.5">
          <summary class="cursor-pointer text-sm text-subtext select-none">Troubleshooting</summary>
          <div class="mt-2 space-y-2">
            <label class="flex gap-2">
              <input type="checkbox" bind:checked={s.preserveSwapchain} class="mt-0.5 shrink-0 accent-[var(--accent)]" />
              <span class="min-w-0">
                <span class="block text-sm leading-snug text-subtext">Game crashes on start</span>
                <span class="block text-[11px] leading-snug text-muted">Keep the swapchain image count (some native Vulkan games).</span>
              </span>
            </label>
            <label class="flex gap-2">
              <input type="checkbox" bind:checked={s.noVsyncOverride} class="mt-0.5 shrink-0 accent-[var(--accent)]" />
              <span class="min-w-0">
                <span class="block text-sm leading-snug text-subtext">Don't force vsync</span>
                <span class="block text-[11px] leading-snug text-muted">Only if the game misbehaves with it — pacing relies on vsync.</span>
              </span>
            </label>
            <label class="flex gap-2">
              <input type="checkbox" bind:checked={s.noFp16} class="mt-0.5 shrink-0 accent-[var(--accent)]" />
              <span class="min-w-0">
                <span class="block text-sm leading-snug text-subtext">Artifacts or slow on an older GPU</span>
                <span class="block text-[11px] leading-snug text-muted">Disable half-precision (FP16) shaders.</span>
              </span>
            </label>
          </div>
        </details>

        {#if st}
          <p class="text-[11px] leading-snug text-muted">
            {#if !st.dll}
              <span class="text-red">Lossless.dll wasn't found, so this can't run yet.</span>
            {:else if dllForEnv}
              Per-game mode skips conf.toml, so the DLL path is passed along as
              <span class="font-mono">LSFGVK_DLL_PATH</span>.
            {:else}
              lsfg-vk finds Lossless.dll on its own here.
            {/if}
          </p>
        {/if}
      </div>
    </div>
  {:else if s.mode === "auto"}
    <div class="space-y-2 text-xs">
      <p class="text-muted">
        Nothing is added to the launch command; lsfg-vk turns on when the game matches a profile's
        <span class="font-mono">active_in</span> list in conf.toml.
      </p>
      {#if autoProfiles.length}
        <ul class="space-y-1">
          {#each autoProfiles as p (p.name)}
            <li class="flex items-center gap-2 rounded-lg border border-border/60 px-3 py-1.5">
              <span class="min-w-0 flex-1 truncate text-subtext">{p.name}</span>
              <span class="truncate font-mono text-[11px] text-muted">{p.active_in.join(", ")}</span>
            </li>
          {/each}
        </ul>
        {#if autoMatch}
          <p class="flex items-center gap-1.5 text-green">
            <CheckCircle size={13} weight="fill" /> This game's executable matches “{autoMatch.name}”.
          </p>
        {:else if exe}
          <p class="text-yellow">This game's executable matches none of them — pick a profile instead.</p>
        {/if}
      {:else}
        <p class="text-yellow">
          None of your profiles has an <span class="font-mono">active_in</span> rule, so nothing will happen
          — pick a profile instead.
        </p>
      {/if}
    </div>
  {:else}
    <p class="text-xs text-muted">
      Adds <span class="font-mono">DISABLE_LSFGVK=1</span>: frame generation stays off for this game even if
      a profile's <span class="font-mono">active_in</span> rule would match it.
    </p>
  {/if}

  <!-- Frame pacing -->
  {#if generates}
    <div class="flex flex-wrap items-center gap-x-2 gap-y-1 rounded-lg bg-mantle/60 px-3 py-2 text-xs text-muted">
      <span>Smoothest when frames land on your refresh rate. On a</span>
      <SelectField label="Display refresh rate" value={refresh} options={REFRESH} onValueChange={(v) => (refresh = v)} width="w-24" />
      <span>display at {effMultiplier}×, cap the game at
        <span class="font-medium text-subtext">{baseCap} fps</span> (in-game limiter or MangoHud's FPS limit).</span>
    </div>
  {/if}

  <!-- Pairing with OptiScaler -->
  {#if s.mode !== "off"}
    <div class="space-y-2 rounded-lg border border-border/60 p-3">
      <p class="flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted">
        <Sparkle size={12} /> Pair with OptiScaler
      </p>
      {#if app.env["PROTON_USE_OPTISCALER"]?.enabled && app.optiFgOn}
        <p class="flex items-start gap-1.5 text-xs text-yellow">
          <WarningCircle size={14} class="mt-px shrink-0" />
          <span>
            OptiScaler's own frame generation is on too, so frames would be generated twice — more artifacts
            and latency. Keep OptiScaler for upscaling and let Lossless Scaling generate the frames.
          </span>
        </p>
        <button
          onclick={() => app.disableOptiFg()}
          class="rounded-lg border border-accent/40 bg-accent/5 px-2.5 py-1 text-xs font-medium text-accent transition hover:bg-accent/10"
        >
          Turn off OptiScaler frame generation
        </button>
      {:else if app.env["PROTON_USE_OPTISCALER"]?.enabled}
        <p class="flex items-start gap-1.5 text-xs text-subtext">
          <CheckCircle size={14} weight="fill" class="mt-px shrink-0 text-green" />
          <span>
            Good pairing: OptiScaler upscales (FSR 4 / XeSS) for a higher base frame rate, Lossless Scaling
            multiplies it.
          </span>
        </p>
        <button onclick={() => app.openOptiFromLsfg()} class="text-xs text-accent hover:underline">
          Configure OptiScaler…
        </button>
      {:else}
        <p class="text-xs text-muted">
          Frame generation looks best from a high base frame rate. In DLSS/FSR/XeSS games, OptiScaler can
          upscale first (e.g. DLSS → FSR 4), then Lossless Scaling multiplies the result. Use OptiScaler for
          upscaling only — not its frame generation.
        </p>
        <div class="flex flex-wrap gap-2">
          <button
            onclick={() => app.enableOptiScaler()}
            class="rounded-lg border border-accent/40 bg-accent/5 px-2.5 py-1 text-xs font-medium text-accent transition hover:bg-accent/10"
          >
            Also inject OptiScaler
          </button>
          <button
            onclick={() => app.openOptiFromLsfg()}
            class="rounded-lg border border-border px-2.5 py-1 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
          >
            Configure OptiScaler…
          </button>
        </div>
      {/if}
      {#if app.env["PROTON_MLFG_UPGRADE"]?.enabled}
        <p class="text-[11px] leading-snug text-muted">
          <span class="font-mono">PROTON_MLFG_UPGRADE</span> is on: if you also enable the game's own FSR frame
          generation in its settings, turn one of them off.
        </p>
      {/if}
    </div>
  {/if}

  <!-- Result -->
  <div class="space-y-1.5">
    <p class="text-[11px] font-medium uppercase tracking-wider text-muted">Result</p>
    <p class="overflow-x-auto rounded-lg bg-mantle p-2 font-mono text-xs break-all text-muted">
      {preview || "(nothing added — conf.toml decides)"}
    </p>
    <p class="text-[11px] leading-snug text-muted">
      Works on Vulkan rendering — almost every Proton game, since DXVK and vkd3d-proton turn D3D into Vulkan.
      OpenGL and WineD3D games aren't affected.
    </p>
  </div>

  <div class="sticky bottom-0 -mx-1 flex justify-end bg-surface-solid px-1 pb-1 pt-2">
    <button
      onclick={apply}
      disabled={s.mode === "profile" && !s.profile}
      class="rounded-lg px-3 py-1.5 text-sm font-medium transition active:scale-95 disabled:opacity-40"
      style="background: var(--accent); color: var(--on-accent)">Apply</button
    >
  </div>
</div>
