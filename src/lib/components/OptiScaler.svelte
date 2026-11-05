<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import SelectField from "./SelectField.svelte";
  import KeyCapture from "./KeyCapture.svelte";
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
  import { openUrl } from "$lib/util";
  import { autofocus } from "$lib/actions";
  import type { OptiscalerChannel, OptiscalerInstall } from "$lib/types";
  import { tick, untrack } from "svelte";
  import {
    ArrowSquareOut,
    ArrowClockwise,
    CloudArrowDown,
    CheckCircle,
    WarningCircle,
    ArrowCircleUp,
    CaretDown,
    Package,
    Wrench,
  } from "phosphor-svelte";
  import Switch from "./Switch.svelte";

  let { onapply }: { onapply?: () => void } = $props();

  // -------------------------- OptiScaler upgrade --------------------------
  // Fetch the latest OptiScaler release from GitHub and extract it into the
  // selected game's folder — the one action here that writes into a game's
  // own directory rather than just building a command string (see
  // optiscaler_upgrade.rs's doc comment for the full rationale). Only offered
  // when the folder already shows signs of an OptiScaler install: this is a
  // refresh, never an injection into a game that isn't using it. And only of a
  // *manual* install — PROTON_USE_OPTISCALER injects its own copy from the
  // prefix and never writes the game folder, so with it on, a fetch here just
  // stacks a second build (lint.rs's optiscaler-double-install).
  let appId = $derived(app.selectedAppId);
  $effect(() => {
    if (appId != null) app.requestOptiscalerStatus(appId);
  });
  let status = $derived(appId == null ? undefined : app.optiscalerStatusFor(appId));
  let statusLoading = $derived(appId != null && app.optiscalerStatusLoading[String(appId)] === true);

  // Installed build vs upstream, from the library-wide overview
  // (optiscaler_versions.rs) — one shared check, not one per open.
  let installs = $derived(appId == null ? [] : app.optiscalerInstallsFor(appId));
  let manualInstall = $derived(installs.find((i) => i.kind === "manual"));
  let protonInstall = $derived(installs.find((i) => i.kind === "proton"));
  /** `PROTON_USE_OPTISCALER=<version>` pins proton-cachyos to that build. */
  let protonPin = $derived.by(() => {
    const v = app.env["PROTON_USE_OPTISCALER"]?.value.trim() ?? "";
    return app.optiInjected && v !== "1" && v !== "" ? v : null;
  });

  /** Why the fetch can't run as things stand, or null. */
  let blocked = $derived(
    app.optiInjected ? "injected" : (status?.proxies.length ?? 0) > 1 ? "stacked" : null,
  );

  let confirmOpen = $state(false);
  let fetchButton = $state<HTMLButtonElement | null>(null);

  const CHANNELS: { value: OptiscalerChannel; label: string }[] = [
    { value: "stable", label: "Stable" },
    { value: "nightly", label: "Nightly" },
  ];

  function openConfirm() {
    app.requestOptiscalerLatest();
    confirmOpen = true;
  }

  /** Back to the button, with focus on it — the panel's buttons are gone. */
  async function cancelConfirm() {
    confirmOpen = false;
    await tick();
    fetchButton?.focus();
  }

  async function doFetch() {
    if (appId == null) return;
    try {
      const result = await app.fetchOptiscalerUpgrade(appId);
      confirmOpen = false;
      const kept = result.ini_preserved ? " — kept your existing OptiScaler.ini" : "";
      toast.success(
        `Installed OptiScaler ${result.tag} as ${result.dll_name}: ${result.files_written} files written${kept}`,
      );
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
  /** Folded unless a fix is already on — they're for a problem you have. */
  let fixesOpen = $state(OPTI_FIXES.some((f) => seed.fixes[f.id]));

  /** The ini entries Apply writes, one per line — easier to scan than one
   *  long semicolon string. */
  let entries = $derived(config ? config.split(";").filter(Boolean) : []);

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

  /** The builder differs from what's in the launch command. */
  let pending = $derived(
    config !== sourceConfig || proxy !== sourceProxy || !app.optiInjected,
  );

  function apply() {
    app.applyOptiScaler(config, proxy);
    toast.success("OptiScaler config applied");
    onapply?.();
  }
</script>

<div class="grid gap-5 md:grid-cols-[minmax(0,1fr)_16rem]">
  <!-- ─────────────── settings, in the order people reach for them ─────────────── -->
  <div class="min-w-0 space-y-3">
    {#if appId != null}
      {@render installStrip()}
    {/if}

    <section class="space-y-2 rounded-xl border border-border/60 p-3">
      {@render heading("Upscaler", "Which upscaler OptiScaler swaps in, per graphics API. Most games are DirectX 12.")}
      {@render pick("DirectX 12", DX12_UPSCALERS, () => c.dx12Upscaler, (v) => (c.dx12Upscaler = v))}
      {@render pick("DirectX 11", DX11_UPSCALERS, () => c.dx11Upscaler, (v) => (c.dx11Upscaler = v))}
      {@render pick("Vulkan", VULKAN_UPSCALERS, () => c.vulkanUpscaler, (v) => (c.vulkanUpscaler = v))}
    </section>

    <section class="space-y-2 rounded-xl border border-border/60 p-3">
      {@render toggleHead(
        "Frame generation (OptiFG)",
        () => c.frameGenOn,
        (v) => (c.frameGenOn = v),
      )}
      {#if app.lsfgActive}
        <!-- Two frame generators in one chain compound artifacts and latency;
             lint.rs's lsfg-double-framegen flags the same thing after Apply. -->
        <p class="text-[11px] leading-snug {c.frameGenOn ? 'text-yellow' : 'text-muted'}">
          Lossless Scaling is generating frames for this game{c.frameGenOn
            ? " — turn this off, or frames get generated twice"
            : ", so leave this off and use OptiScaler for upscaling"}.
          <button class="text-accent hover:underline" onclick={() => app.openLsfgFromOpti()}
            >Lossless Scaling settings…</button
          >
        </p>
      {:else}
        <p class="text-[11px] leading-snug text-muted">
          Or leave this off and pair OptiScaler's upscaling with
          <button class="text-accent hover:underline" onclick={() => app.openLsfgFromOpti()}
            >Lossless Scaling frame generation…</button
          >
        </p>
      {/if}
      {#if c.frameGenOn}
        <div class="space-y-2 border-l-2 border-accent/30 pl-3">
          {@render pick("Input", FG_INPUTS, () => c.fgInput, (v) => (c.fgInput = v))}
          {@render pick("Output", FG_OUTPUTS, () => c.fgOutput, (v) => (c.fgOutput = v))}
        </div>
      {/if}
    </section>

    <section class="space-y-3 rounded-xl border border-border/60 p-3">
      {@render heading("Image quality")}
      <div class="space-y-2">
        {@render toggleHead("Sharpening", () => c.sharpenOn, (v) => (c.sharpenOn = v))}
        {#if c.sharpenOn}
          <div class="space-y-2 border-l-2 border-accent/30 pl-3">
            {@render slider("Amount", 0, 1, 0.05, () => c.sharpness, (v) => (c.sharpness = v), "")}
            {@render pick("Filter", SHARPEN_SHADERS, () => c.sharpenShader, (v) => (c.sharpenShader = v))}
          </div>
        {/if}
      </div>
      <div class="space-y-2 border-t border-border/60 pt-3">
        {@render toggleHead("Output scaling", () => c.outputScalingOn, (v) => (c.outputScalingOn = v))}
        {#if c.outputScalingOn}
          <div class="border-l-2 border-accent/30 pl-3">
            {@render slider("Multiplier", 0.5, 3, 0.1, () => c.outputScalingMult, (v) => (c.outputScalingMult = v), "×")}
          </div>
        {/if}
      </div>
      <div class="space-y-2 border-t border-border/60 pt-3">
        {@render toggleHead("Override DLSS render preset", () => c.dlssPresetOn, (v) => (c.dlssPresetOn = v))}
        {#if c.dlssPresetOn}
          <label class="flex items-center justify-between gap-2 border-l-2 border-accent/30 pl-3">
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
      <div class="border-t border-border/60 pt-3">
        {@render toggleHead(
          "Unlock DLSS / RT on Radeon",
          () => c.spoofUnlock,
          (v) => (c.spoofUnlock = v),
          "Streamline GPU spoofing, so RE Engine / Streamline games expose their DLSS and ray-tracing options for OptiScaler to translate. On by default upstream — set it here when a game hides them.",
        )}
      </div>
    </section>

    <!--
      Compatibility fixes, labelled by symptom. These are the settings people
      come here for when a game misbehaves, and none of them is guessable from
      its ini key — so the key is not shown here; it is in the Result pane for
      anyone who wants to check. Folded by default: each trades something away.
    -->
    <section class="rounded-xl border border-border/60 p-3">
      <button
        class="flex w-full items-center gap-2 text-left"
        aria-expanded={fixesOpen}
        onclick={() => (fixesOpen = !fixesOpen)}
      >
        <Wrench size={14} class="text-muted" />
        <span class="text-sm font-medium text-text">Fix a problem</span>
        {#if fixCount}
          <span class="rounded-full bg-accent/15 px-1.5 text-[10px] text-accent">{fixCount} on</span>
        {/if}
        <span class="ml-auto text-[11px] text-muted">{OPTI_FIXES.length} fixes</span>
        <CaretDown size={13} class="text-muted transition {fixesOpen ? 'rotate-180' : ''}" />
      </button>
      {#if fixesOpen}
        <p class="mt-2 text-[11px] leading-snug text-muted">
          Only turn these on for a problem you're actually seeing — each one trades something
          away.
        </p>
        <div class="mt-2 space-y-2.5">
          {#each OPTI_FIXES as f (f.id)}
            {@render toggleHead(
              f.label,
              () => !!c.fixes[f.id],
              (v) => (c.fixes[f.id] = v),
              f.note,
            )}
          {/each}
        </div>
      {/if}
    </section>

    <section class="space-y-2 rounded-xl border border-border/60 p-3">
      {@render heading("Loading")}
      {@render pick("Inject as", PROXY_DLLS, () => proxy, (v) => (proxy = v))}
      <p class="text-[11px] leading-snug text-muted">
        Change this if OptiScaler never loads at all — some games already own
        <span class="font-mono">dxgi.dll</span>.
      </p>
      <div class="flex items-start justify-between gap-2">
        <span class="pt-1 text-sm text-subtext">In-game menu hotkey</span>
        <!-- OptiScaler runs inside the game under Wine and reads a raw
             Windows virtual-key code, never a name; -1 turns the hotkey off. -->
        <KeyCapture
          label="In-game menu hotkey"
          format="vk"
          defaultLabel="Insert"
          disableValue="-1"
          value={c.menuShortcutKey}
          onchange={(v) => (c.menuShortcutKey = v)}
        />
      </div>
    </section>
  </div>

  <!-- ─────────────── what Apply will write ─────────────── -->
  <aside class="space-y-3 self-start md:sticky md:top-0">
    <div class="space-y-2 rounded-xl border border-border/60 bg-mantle/40 p-3">
      <p class="flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted">
        Writes
        {#if pending}
          <span class="ml-auto inline-flex items-center gap-1 normal-case tracking-normal text-peach"
            ><span class="size-1.5 rounded-full bg-peach"></span> not applied</span
          >
        {:else}
          <span class="ml-auto inline-flex items-center gap-1 normal-case tracking-normal text-green"
            ><CheckCircle size={11} weight="fill" /> applied</span
          >
        {/if}
      </p>
      <ul class="space-y-0.5 font-mono text-[11px] leading-snug">
        {#if !app.optiInjected}
          <li class="break-all text-green">+ PROTON_USE_OPTISCALER=1</li>
        {/if}
        {#if proxy}
          <li class="break-all text-subtext">PROTON_OPTISCALER_NAME={proxy}</li>
        {/if}
        {#each entries as e (e)}
          <li class="break-all text-subtext">{e}</li>
        {:else}
          <li class="text-muted">OptiScaler defaults — no ini overrides</li>
        {/each}
      </ul>
      {#if c.passthrough.length}
        <p class="text-[11px] leading-snug text-muted">
          {c.passthrough.length} setting{c.passthrough.length === 1 ? "" : "s"} this builder doesn't
          model {c.passthrough.length === 1 ? "is" : "are"} kept as-is.
        </p>
      {/if}
      <p class="text-[11px] leading-snug text-muted">
        Only what you change is written; everything else stays at OptiScaler's defaults.
      </p>
    </div>
    <button
      onclick={apply}
      class="w-full rounded-lg px-3 py-2 text-sm font-medium transition active:scale-95"
      style="background: var(--accent); color: var(--on-accent)">Apply to launch command</button
    >
  </aside>
</div>

{#snippet confirmPanel(installDir: string, proxies: string[], strayDll: boolean)}
  <!-- Inline, not a second <Dialog>: this panel already lives inside the
       overlay-builder dialog, and a dialog stacked on a dialog is the #63
       click-dead pattern. Escape cancels this step, not the whole builder. -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    role="group"
    aria-label="Fetch latest OptiScaler build"
    onkeydown={(e) => {
      if (e.key !== "Escape" || app.optiscalerFetchBusy) return;
      e.stopPropagation();
      cancelConfirm();
    }}
    class="space-y-3 rounded-lg border border-accent/30 bg-accent/5 p-3 text-sm"
  >
    <div class="flex items-center justify-between gap-2">
      <span class="text-xs text-muted">Build</span>
      <div role="radiogroup" aria-label="OptiScaler build channel" class="inline-flex rounded-lg border border-border/60 p-0.5 text-xs">
        {#each CHANNELS as ch (ch.value)}
          <button
            role="radio"
            aria-checked={app.optiscalerChannel === ch.value}
            disabled={app.optiscalerFetchBusy}
            onclick={() => app.setOptiscalerChannel(ch.value)}
            class="rounded-md px-2.5 py-0.5 transition disabled:opacity-40 {app.optiscalerChannel === ch.value
              ? 'bg-accent font-medium text-on-accent'
              : 'text-muted hover:text-text'}"
          >
            {ch.label}
          </button>
        {/each}
      </div>
    </div>
    {#if blocked === "injected"}
      <div class="space-y-2 rounded-lg border border-yellow/40 bg-yellow/5 p-3 text-xs leading-snug text-yellow">
        <p class="flex items-start gap-1.5">
          <WarningCircle size={14} class="mt-px shrink-0" />
          <span>
            <span class="font-mono">PROTON_USE_OPTISCALER</span> is on, so Proton already injects its own
            OptiScaler from the prefix. Upgrading the copy in the game folder would leave two builds
            loading in one process. Turn injection off to keep using this manual install (the
            builder's <span class="font-mono">PROTON_OPTISCALER_CONFIG</span> then has no effect), or
            remove the manual install to use Proton's.
          </span>
        </p>
        <button
          onclick={() => app.toggleEnv("PROTON_USE_OPTISCALER")}
          class="rounded-lg border border-accent/40 bg-accent/5 px-2.5 py-1 font-medium text-accent transition hover:bg-accent/10"
        >
          Turn off PROTON_USE_OPTISCALER
        </button>
      </div>
    {:else if blocked === "stacked"}
      <p class="flex items-start gap-1.5 rounded-lg border border-red/40 bg-red/5 p-3 text-xs leading-snug text-red">
        <WarningCircle size={14} class="mt-px shrink-0" />
        <span>
          {#each proxies as p, i (p)}{i ? (i === proxies.length - 1 ? " and " : ", ") : ""}<span
              class="font-mono">{p}</span
            >{/each} are each an OptiScaler build, so two copies load at once. Remove all but one, then fetch
          again — upgrading one would leave the other on the old build.
        </span>
      </p>
    {:else if app.optiscalerLatestLoading}
      <p class="flex items-center gap-1.5 text-xs text-muted">
        <ArrowClockwise size={12} class="animate-spin" /> Checking the latest release…
      </p>
    {:else if app.optiscalerLatestError}
      <p class="text-xs text-red">Couldn't reach GitHub: {app.optiscalerLatestError}</p>
    {:else if app.optiscalerLatest}
      {@const latest = app.optiscalerLatest}
      <div class="space-y-1.5 rounded-lg border border-border/60 bg-surface-2/60 p-3 text-xs">
        <div class="flex items-center justify-between gap-2">
          <span class="text-muted">Source</span>
          <button
            onclick={() => openUrl(latest.html_url)}
            class="inline-flex items-center gap-1 font-medium text-accent hover:underline"
          >
            {latest.repo} {latest.tag} <ArrowSquareOut size={11} />
          </button>
        </div>
        <div class="flex items-center justify-between gap-2">
          <span class="text-muted">Asset</span>
          <span class="truncate font-mono text-subtext">{latest.asset_name}</span>
        </div>
        <div class="flex items-center justify-between gap-2">
          <span class="text-muted">Destination</span>
          <span class="truncate font-mono text-subtext">{installDir}</span>
        </div>
        <div class="flex items-center justify-between gap-2">
          <span class="text-muted">OptiScaler.dll as</span>
          <span class="truncate font-mono text-subtext">{proxies[0] ?? "OptiScaler.dll"}</span>
        </div>
      </div>
      <p class="text-xs leading-snug text-muted">
        Downloads that archive and extracts every file it contains into the folder above,
        overwriting anything with the same name — <span class="font-medium text-subtext"
          >except an existing <span class="font-mono">OptiScaler.ini</span>, which is left
          untouched.</span
        >
        {#if proxies.length === 1}
          The new <span class="font-mono">OptiScaler.dll</span> replaces
          <span class="font-mono">{proxies[0]}</span>, the copy the game actually loads.
        {/if}
        No checksum is published for this release; integrity rests on HTTPS + fetching directly
        from the project's own GitHub Releases.
      </p>
      {#if strayDll}
        <p class="text-xs leading-snug text-muted">
          The <span class="font-mono">OptiScaler.dll</span> already in this folder isn't loaded by
          anything — likely left by an earlier fetch. It's left alone; delete it if you like.
        </p>
      {/if}
      {#if latest.channel === "nightly"}
        <p class="text-xs leading-snug text-yellow">
          Nightly is an untested daily build of OptiScaler's development branch — expect
          regressions. Its file layout and <span class="font-mono">OptiScaler.ini</span> keys can
          differ from stable; switch back and fetch again to return to the stable build.
        </p>
      {/if}
    {/if}

    <div class="flex justify-end gap-2">
      <button
        use:autofocus
        onclick={cancelConfirm}
        disabled={app.optiscalerFetchBusy}
        class="rounded-lg px-3 py-1.5 text-xs text-muted hover:text-text disabled:opacity-40"
      >
        Cancel
      </button>
      <button
        onclick={doFetch}
        disabled={!app.optiscalerLatest || app.optiscalerFetchBusy || blocked != null}
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
{/snippet}

{#snippet versionLine(i: OptiscalerInstall)}
  <p class="flex flex-wrap items-center gap-x-1.5 text-xs">
    <span class="text-muted">Installed</span>
    <span class="font-mono text-subtext"
      >{i.build?.version ?? "unknown"}{i.build?.built ? ` (${i.build.built})` : ""}</span
    >
    {#if i.freshness === "current"}
      <span class="inline-flex items-center gap-1 text-green"><CheckCircle size={12} weight="fill" /> up to date</span>
    {:else if i.freshness === "outdated"}
      <span class="inline-flex items-center gap-1 font-medium text-accent"
        ><ArrowCircleUp size={12} weight="fill" /> {i.latest} available</span
      >
    {:else if i.freshness === "ahead"}
      <span class="text-yellow">newer than any published build</span>
    {:else if i.freshness === "proton-refresh"}
      <span class="text-muted">· Proton swaps in {i.latest} on the next launch</span>
    {:else if i.freshness === "proton-waiting"}
      <span class="text-muted">· proton-upscalers' newest; OptiScaler {i.latest} isn't packaged yet</span>
    {:else if app.optiscalerOverviewLoading}
      <ArrowClockwise size={11} class="animate-spin text-muted" />
    {:else}
      <span class="text-muted">· couldn't compare with upstream</span>
    {/if}
  </p>
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

{#snippet heading(title: string, note = "")}
  <div>
    <p class="text-sm font-medium text-text">{title}</p>
    {#if note}<p class="text-[11px] leading-snug text-muted">{note}</p>{/if}
  </div>
{/snippet}

<!-- One on/off setting: the app's Switch, not a bare checkbox, so this dialog
     reads like every option row behind it. -->
{#snippet toggleHead(label: string, get: () => boolean, set: (v: boolean) => void, note = "")}
  {@const id = `opti-${label.replace(/\W+/g, "-")}`}
  <div class="flex items-start gap-3">
    <div class="min-w-0 flex-1">
      <span {id} class="block text-sm leading-snug {get() ? 'text-text' : 'text-subtext'}">{label}</span>
      {#if note}<span class="block text-[11px] leading-snug text-muted">{note}</span>{/if}
    </div>
    <Switch checked={get()} onchange={() => set(!get())} labelledby={id} />
  </div>
{/snippet}

{#snippet slider(
  label: string,
  min: number,
  max: number,
  step: number,
  get: () => string,
  set: (v: string) => void,
  unit: string,
)}
  <label class="flex items-center gap-3">
    <span class="text-sm text-subtext">{label}</span>
    <input
      type="range"
      {min}
      {max}
      {step}
      value={get()}
      oninput={(e) => set(e.currentTarget.value)}
      aria-label={label}
      class="ml-auto w-36 accent-[var(--accent)]"
    />
    <span class="w-9 text-right font-mono text-xs text-muted">{get()}{unit}</span>
  </label>
{/snippet}

<!--
  Which OptiScaler this game actually loads, in one line. It used to be a full
  card at the top of the dialog, ahead of the settings people open it for; the
  fetch flow (a rare, write-to-disk action) now unfolds from here on demand.
-->
{#snippet installStrip()}
  <section class="space-y-2 rounded-xl border border-border/60 bg-mantle/40 p-3">
    <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
      <Package size={15} class="text-muted" />
      {#if statusLoading || !status}
        <span class="flex items-center gap-1.5 text-xs text-muted">
          <ArrowClockwise size={12} class="animate-spin" /> Checking this game's folder…
        </span>
      {:else if app.optiInjected}
        <span class="text-sm text-text">Proton injects OptiScaler</span>
        <span class="text-xs text-muted">from the prefix, re-synced every launch</span>
      {:else if status.found}
        <span class="text-sm text-text">Manual install</span>
        <span class="min-w-0 truncate font-mono text-xs text-muted" title={status.install_dir ?? ""}
          >{status.proxies.length === 1 ? status.proxies[0] : "game folder"}</span
        >
      {:else}
        <span class="text-sm text-subtext">Not active yet</span>
        <span class="text-xs text-muted">— Apply turns on Proton's injected copy</span>
      {/if}
      <button
        onclick={() => app.openOptiscalerManagerFromBuilder()}
        class="ml-auto text-[11px] text-accent hover:underline">All versions…</button
      >
    </div>

    {#if status && !statusLoading}
      {#if app.optiInjected && protonInstall}
        {@render versionLine(protonInstall)}
        {#if protonPin}
          <p class="text-[11px] leading-snug text-yellow">
            Pinned to <span class="font-mono">{protonPin}</span> by
            <span class="font-mono">PROTON_USE_OPTISCALER</span> — set it to
            <span class="font-mono">1</span> to follow proton-upscalers' newest build.
          </p>
        {/if}
      {:else if status.found && manualInstall}
        {@render versionLine(manualInstall)}
      {:else if !status.found && protonInstall}
        {@render versionLine(protonInstall)}
      {/if}

      {#if status.found && status.install_dir}
        {#if app.optiInjected}
          <!-- Both at once: say so here, not only behind the fetch button. -->
          <p class="flex items-start gap-1.5 text-[11px] leading-snug text-yellow">
            <WarningCircle size={12} class="mt-px shrink-0" />
            A manual install is also in the game folder, so two builds may load. Remove one.
          </p>
        {/if}
        {#if confirmOpen}
          {@render confirmPanel(status.install_dir, status.proxies, status.stray_dll)}
        {:else}
          <button
            bind:this={fetchButton}
            onclick={openConfirm}
            class="inline-flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
          >
            <CloudArrowDown size={13} /> Update the manual install…
            {#if app.optiscalerChannel === "nightly"}
              <span class="rounded bg-yellow/15 px-1 text-[10px] uppercase tracking-wide text-yellow">nightly</span>
            {/if}
          </button>
        {/if}
      {:else if !status.install_dir}
        <p class="text-[11px] text-muted">This game's install folder couldn't be resolved.</p>
      {/if}
    {/if}
  </section>
{/snippet}
