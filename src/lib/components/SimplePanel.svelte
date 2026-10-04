<script lang="ts">
  import { tick } from "svelte";
  import { app } from "$lib/state.svelte";
  import { irrelevance } from "$lib/util";
  import { prefersReducedMotion } from "$lib/motion.svelte";
  import Switch from "./Switch.svelte";
  import ModeToggle from "./ModeToggle.svelte";
  import GameRuntimePanel from "./GameRuntimePanel.svelte";
  import CurrentGameCard from "./CurrentGameCard.svelte";
  import Recipes from "./Recipes.svelte";
  import ActiveOptions from "./ActiveOptions.svelte";
  import {
    Rocket,
    MagicWand,
    Gauge,
    Sparkle,
    Sun,
    DiscordLogo,
    GlobeHemisphereWest,
    SlidersHorizontal,
    CubeTransparent,
    Drop,
    FilmStrip,
  } from "phosphor-svelte";
  import { buildLsfg, defaultLsfg, envDllPath } from "$lib/lsfg";
  import type { Component } from "svelte";

  /**
   * Simple mode: a curated grid of the options most people actually reach for,
   * each a plain toggle over the *same* catalog keys Advanced mode exposes. This
   * is a view, not a second source of truth — every card drives `app.toggleEnv`
   * / `app.toggleWrap`, so switching modes never loses or diverges state.
   *
   * The MangoHud / OptiScaler dialogs are the real builders. They mount once at
   * the app root (`OverlayBuilders`), not here — see `app.mangoBuilderOpen`.
   */

  interface Card {
    id: string;
    title: string;
    blurb: string;
    icon: Component;
    /** Hidden unless relevant to the detected/opt-in hardware. */
    gpu?: string | null;
    needs?: string[];
    /** Catalog env keys this card turns on together, with the value to set. */
    env?: [string, string][];
    /** Catalog wrapper keys this card turns on together. */
    wrap?: [string, string][];
    /**
     * A literal token this card keeps present in the free-form game-arguments
     * field (space-joined), for fixes that only work as a launch argument
     * rather than an env var or wrapper — e.g. RE Engine's Wine-detection
     * bypass. Not catalog-backed like `env`/`wrap`; `app.gameArgs` is a plain
     * free-text field, so this only ever appends/removes an exact substring.
     */
    gameArg?: string;
    /** Opens one of the builder dialogs in addition to the toggle. */
    configure?: "mango" | "opti" | "vk" | "lsfg";
    /** For a card whose on-state isn't "these keys are all set" — overrides
     *  the env/wrap/gameArg bundle for both reading and toggling. */
    custom?: { isOn: () => boolean; set: (on: boolean) => void };
  }

  /** Each builder dialog's open flag. They mount once at the app root. */
  const OPEN_BUILDER = {
    mango: () => (app.mangoBuilderOpen = true),
    opti: () => (app.optiBuilderOpen = true),
    vk: () => (app.vkBuilderOpen = true),
    lsfg: () => (app.lsfgBuilderOpen = true),
  } as const;

  /**
   * Lossless Scaling's switch. On picks the least-surprising setup: the user's
   * own first lsfg-vk profile when they have one (they already tuned it in
   * lsfg-vk-ui), else 2× per-game settings. Off clears every lsfg-vk variable
   * the builder owns. "Configure…" is where the real choice is made.
   */
  function setLsfg(on: boolean) {
    if (!on) {
      app.applyLsfg([], "disable Lossless Scaling");
      return;
    }
    const st = app.lsfgStatus;
    const s = defaultLsfg();
    const first = st?.profiles[0]?.name;
    if (first) {
      s.mode = "profile";
      s.profile = first;
    } else {
      s.mode = "custom";
    }
    app.applyLsfg(buildLsfg(s, envDllPath(st)), "enable Lossless Scaling");
  }

  // Prefer CachyOS's game-performance when it's installed, else Feral GameMode.
  const perfWrap = $derived(
    app.requiresStatus["game-performance"] ? "game-performance" : "gamemoderun",
  );
  const perfBlurb = $derived(
    perfWrap === "game-performance"
      ? "CachyOS game-performance: performance power profile + gaming scheduler while the game runs."
      : "Feral GameMode: CPU governor + IO priority tuning while the game runs. Safe, universal.",
  );

  let CARDS = $derived<Card[]>([
    {
      id: "performance",
      title: "Performance mode",
      blurb: perfBlurb,
      icon: Rocket,
      wrap: [[perfWrap, ""]],
    },
    {
      id: "fsr4",
      title: "FSR 4 upscaling",
      blurb: "Upgrade the game's FSR to FSR 4 (plus multi-frame generation) for RDNA3/RDNA4.",
      icon: MagicWand,
      needs: ["fsr4"],
      env: [
        ["PROTON_FSR4_UPGRADE", "1"],
        ["PROTON_MLFG_UPGRADE", "1"],
      ],
    },
    {
      id: "raytracing",
      title: "Ray tracing",
      blurb:
        "Force ray tracing on at the vkd3d-proton layer and unblock it on RE Engine games (Resident Evil, Pragmata, Monster Hunter) that hide RT when they detect Wine.",
      icon: CubeTransparent,
      env: [["VKD3D_CONFIG", "dxr"]],
      gameArg: "/WineDetectionEnabled:False",
    },
    {
      id: "overlay",
      title: "Performance overlay",
      blurb: "MangoHud: FPS, frame times, and CPU/GPU stats on screen. Configure what it shows.",
      icon: Gauge,
      wrap: [["mangohud", ""]],
      configure: "mango",
    },
    {
      id: "optiscaler",
      title: "OptiScaler",
      blurb: "Swap a game's DLSS/XeSS/FSR for another upscaler, or inject frame generation.",
      icon: Sparkle,
      env: [["PROTON_USE_OPTISCALER", "1"]],
      configure: "opti",
    },
    {
      id: "vkbasalt",
      title: "Sharpening & post-processing",
      blurb: "vkBasalt: CAS sharpening and SMAA/FXAA on top of the game. It can also run ReShade .fx shaders, so it covers ReShade too. Handy with an upscaler.",
      icon: Drop,
      env: [["ENABLE_VKBASALT", "1"]],
      configure: "vk",
    },
    {
      id: "lsfg",
      title: "Lossless Scaling frame gen",
      blurb:
        "2×–4× frame generation from your Steam copy of Lossless Scaling (lsfg-vk), for any Vulkan/DXVK game. Pairs with OptiScaler upscaling.",
      icon: FilmStrip,
      configure: "lsfg",
      custom: { isOn: () => app.lsfgActive, set: setLsfg },
    },
    {
      id: "hdr",
      title: "HDR output",
      blurb: "Native Wayland driver + DXVK HDR. Needs an HDR display on a Wayland session.",
      icon: Sun,
      needs: ["wayland", "hdr"],
      env: [
        ["PROTON_ENABLE_WAYLAND", "1"],
        ["DXVK_HDR", "1"],
      ],
    },
    {
      id: "discord",
      title: "Discord Rich Presence",
      blurb: "CachyOS rpc-bridge: show the game you're playing in Discord for Proton games.",
      icon: DiscordLogo,
      env: [["PROTON_DISCORD_BRIDGE", "1"]],
    },
  ]);

  function relevant(c: Card): boolean {
    return irrelevance(app.hwCaps, c.gpu ?? null, c.needs ?? []) === null;
  }
  let cards = $derived(CARDS.filter(relevant));

  /** A card is "on" when every key (and game-arg token) it owns is present. */
  function isActive(c: Card): boolean {
    if (c.custom) return c.custom.isOn();
    const envOn = (c.env ?? []).every(([k]) => app.env[k]?.enabled);
    const wrapOn = (c.wrap ?? []).every(([k]) => app.wrap[k]?.enabled);
    const argOn = !c.gameArg || app.hasGameArg(c.gameArg);
    return envOn && wrapOn && argOn;
  }

  /**
   * One card, one undo entry. This used to drive `app.toggleEnv`/`toggleWrap`
   * per key, each of which flushes its own history entry — so undoing the FSR 4
   * card (two env vars) reverted half of it, and the ray-tracing card's game-arg
   * token was never recorded as its own action at all.
   */
  function toggle(c: Card) {
    const on = !isActive(c);
    if (c.custom) {
      c.custom.set(on);
      return;
    }
    app.applyBundle(`${on ? "enable" : "disable"} ${c.title}`, on, {
      env: c.env,
      wrappers: c.wrap,
      gameArg: c.gameArg,
    });
  }

  const gp = $derived(app.store.global_profile);

  // A jump to Recipes / Active options / the game panel (palette, notice links,
  // ActiveOptions' own links) lands here in Simple mode: open it and scroll to it.
  $effect(() => {
    const target = app.simpleAnchor;
    if (!target) return;
    void target.nonce;
    void tick().then(() =>
      document.getElementById(target.id)?.scrollIntoView({
        block: "start",
        behavior: prefersReducedMotion() ? "auto" : "smooth",
      }),
    );
  });
</script>

<div class="space-y-4">
  <!-- Which game this is — Advanced shows it in the NavRail, which Simple lacks. -->
  <CurrentGameCard details />

  <!-- Steam vs umu, then the game & runtime block (shared with Advanced). -->
  <div class="flex items-center justify-between">
    <h2 class="text-sm font-medium tracking-wide text-text">Launch mode</h2>
    <ModeToggle />
  </div>
  <div id="simple-game" class="scroll-mt-4"><GameRuntimePanel /></div>

  <!-- Default profile -->
  <section class="card flex items-center gap-3 p-4">
    <GlobeHemisphereWest size={22} class="shrink-0 text-accent" />
    <div class="min-w-0 flex-1">
      <p class="text-sm font-medium text-text">Default profile</p>
      <p class="text-xs text-muted">
        {#if gp}
          Your reusable defaults — {gp.env.length} env · {gp.wrappers.length} wrappers.
        {:else}
          Save a set of options as your default, then apply it to any game in one click.
        {/if}
      </p>
    </div>
    {#if gp}
      <button
        onclick={() => app.applyGlobalProfile()}
        class="shrink-0 rounded-lg bg-accent px-3 py-1.5 text-xs font-medium text-on-accent transition hover:opacity-90"
      >
        Apply
      </button>
    {/if}
    <button
      onclick={() => app.openSettings("behavior")}
      class="inline-flex shrink-0 items-center gap-1 rounded-lg border border-border px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50"
    >
      <SlidersHorizontal size={13} /> {gp ? "Edit" : "Set up"}
    </button>
  </section>

  <!-- Popular options -->
  <div>
    <p class="mb-2 px-1 text-[11px] font-medium uppercase tracking-wider text-muted">
      Popular options
    </p>
    <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
      {#each cards as c (c.id)}
        {@const active = isActive(c)}
        <section class="card flex flex-col gap-2.5 p-4">
          <div class="flex items-start gap-2.5">
            <c.icon size={20} class="mt-0.5 shrink-0 text-accent" weight="fill" />
            <div class="min-w-0 flex-1">
              <p class="text-sm font-medium text-text">{c.title}</p>
            </div>
            <Switch checked={active} onchange={() => toggle(c)} label={c.title} />
          </div>
          <p class="text-xs leading-relaxed text-muted">{c.blurb}</p>
          {#if c.configure}
            {@const configure = c.configure}
            <button
              onclick={() => OPEN_BUILDER[configure]()}
              class="mt-auto inline-flex w-fit items-center gap-1 rounded-lg border border-border px-2.5 py-1 text-xs text-subtext transition hover:border-accent/50 hover:text-text"
            >
              <SlidersHorizontal size={12} /> Configure…
            </button>
          {/if}
        </section>
      {/each}
    </div>
  </div>

  <!-- Recipes: the same one-click profiles and troubleshooter fixes Advanced has. -->
  <div id="simple-recipes" class="scroll-mt-4"><Recipes /></div>

  <!-- Everything turned on, including options set in Advanced, a recipe or an
       import that no card above shows. -->
  <div id="simple-active" class="scroll-mt-4"><ActiveOptions /></div>

  <p class="px-1 text-xs text-muted">
    Looking for more? Switch to <span class="font-medium text-subtext">Advanced</span> in the header
    for the full catalog and search.
  </p>
</div>
