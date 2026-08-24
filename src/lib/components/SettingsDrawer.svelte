<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { keys } from "$lib/keys.svelte";
  import { THEMES } from "$lib/themes";
  import { fly, fade } from "$lib/motion.svelte";
  import { mergeStyle } from "$lib/util";
  import { Dialog as DialogPrimitive } from "bits-ui";
  import Switch from "./Switch.svelte";
  import {
    GearSix,
    Palette,
    SlidersHorizontal,
    Check,
    X,
    CaretDown,
    FolderOpen,
    Trash,
    Robot,
  } from "phosphor-svelte";
  // Aliased: `open` is already the drawer's own bindable prop.
  import { open as pickPath } from "@tauri-apps/plugin-dialog";
  import { inTauri, ipc } from "$lib/ipc";
  import Badges from "./Badges.svelte";
  import type { Component } from "svelte";
  import type { GpuGen } from "$lib/types";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  // Keep the global key layer quiet while the drawer is up — see Dialog.svelte.
  $effect(() => {
    if (!open) return;
    keys.pushOverlay();
    return () => keys.popOverlay();
  });

  // Sections are collapsible; all start collapsed to keep the drawer tidy.
  let sections = $state({ appearance: false, behavior: false, ai: false, paths: false });

  // Local-LLM connection test (populates the model picker from GET /models).
  let llmModels = $state<string[]>([]);
  let llmTest = $state<"idle" | "loading" | "ok" | "err">("idle");
  let llmTestMsg = $state("");
  async function testLlm() {
    llmTest = "loading";
    llmTestMsg = "";
    try {
      llmModels = await ipc.llmModels();
      llmTest = "ok";
      llmTestMsg = llmModels.length
        ? `${llmModels.length} model${llmModels.length === 1 ? "" : "s"} available`
        : "Connected, but the server lists no models.";
    } catch (e) {
      llmTest = "err";
      llmTestMsg = String(e);
      llmModels = [];
    }
  }

  // Typed rather than inlined in the {#each}, so the tuple widens to GpuGen
  // instead of string and `setGpuGen` keeps its union.
  const GPU_GENS: [GpuGen, string][] = [
    ["", "Not AMD"],
    ["rdna3", "RDNA3"],
    ["rdna4", "RDNA4"],
  ];
</script>

<!--
  The bespoke fly/fade markup is kept verbatim; only the layer behaviour
  (escape stack, focus trap, focus restore, scroll lock) now comes from
  bits-ui. forceMount + an inner {#if open} is what preserves the exit
  transition, since bits-ui unmounts on close by default.

  This drawer used to register its own competing svelte:window Escape handler,
  which is half of the layering bug: with the Save-preset dialog open on top,
  one Escape closed both.
-->
<DialogPrimitive.Root bind:open>
  <DialogPrimitive.Portal>
    <DialogPrimitive.Overlay forceMount>
      {#snippet child({ props })}
        {#if open}
          <div
            {...props}
            class="fixed inset-0 z-[100] bg-black/50 backdrop-blur-sm"
            transition:fade={{ duration: 120 }}
          ></div>
        {/if}
      {/snippet}
    </DialogPrimitive.Overlay>
    <DialogPrimitive.Content forceMount>
      {#snippet child({ props })}
        {#if open}
          <div class="fixed inset-0 z-[100] flex justify-end" role="presentation">
            <div
              {...props}
              class="flex h-full w-[360px] max-w-[90vw] flex-col border-l border-border shadow-2xl"
              style={mergeStyle(props, "background: var(--surface-solid)")}
              transition:fly={{ x: 360, duration: 200 }}
              aria-label="Settings"
            >
      <header class="flex items-center gap-2 border-b border-border px-4 py-3">
        <GearSix size={18} weight="fill" class="text-accent" />
        <h2 class="text-sm font-medium text-text">Settings</h2>
        <button
          onclick={() => (open = false)}
          aria-label="Close settings"
          class="ml-auto grid size-7 place-items-center rounded-lg text-muted transition hover:bg-surface-2 hover:text-text"
        >
          <X size={16} />
        </button>
      </header>

      <div class="flex-1 space-y-6 overflow-y-auto p-4">
        <!-- Appearance -->
        <section>
          {@render sectionHeading(
            Palette,
            "Appearance",
            sections.appearance,
            () => (sections.appearance = !sections.appearance),
          )}
          {#if sections.appearance}
          <div id="drawer-section-appearance" class="mt-2 grid grid-cols-2 gap-1.5">
            {#each THEMES as t (t.id)}
              <button
                onclick={() => app.setTheme(t.id)}
                class="flex items-center gap-1.5 rounded-lg border px-2.5 py-2 text-left text-xs transition {app
                  .store.theme === t.id
                  ? 'border-accent text-text'
                  : 'border-border text-subtext hover:border-accent/50'}"
              >
                {#if app.store.theme === t.id}
                  <Check size={12} class="shrink-0 text-accent" />
                {:else}
                  <span class="size-3 shrink-0"></span>
                {/if}
                <span class="truncate">{t.label}</span>
              </button>
            {/each}
          </div>
          {/if}
        </section>

        <!-- Behavior -->
        <section>
          {@render sectionHeading(
            SlidersHorizontal,
            "Behavior",
            sections.behavior,
            () => (sections.behavior = !sections.behavior),
          )}
          {#if sections.behavior}
          <div id="drawer-section-behavior" class="mt-2 space-y-0.5">
            {@render toggle(
              "Show unsupported options",
              "List recipes that don't match your detected hardware.",
              app.store.show_irrelevant,
              () => app.setShowIrrelevant(!app.store.show_irrelevant),
            )}
            {@render toggle(
              "Show advanced options",
              "Adds the debugging, logging and low-level tuning parameters.",
              app.store.show_advanced,
              () => app.setShowAdvanced(!app.store.show_advanced),
            )}
            {@render toggle(
              "I have an HDR display",
              "Enables HDR recipes. HDR can't be auto-detected.",
              app.store.hdr,
              () => app.setHdr(!app.store.hdr),
            )}
            {@render gpuGen()}
            {@render toggle(
              "Auto-check ProtonDB",
              "Fetch the compatibility tier when a Steam game is selected.",
              app.store.protondb_auto,
              () => app.setProtondbAuto(!app.store.protondb_auto),
            )}
            {@render globalProfile()}
          </div>
          {/if}
        </section>

        <!-- AI assistant -->
        <section>
          {@render sectionHeading(
            Robot,
            "AI assistant",
            sections.ai,
            () => (sections.ai = !sections.ai),
          )}
          {#if sections.ai}
          <div id="drawer-section-ai-assistant" class="mt-2 space-y-0.5">
            {@render toggle(
              "Enable AI log coach",
              "Send a game's Proton log to a local LLM for tuning suggestions. Needs a running OpenAI-compatible server (LM Studio, Ollama, llama.cpp).",
              app.store.llm_enabled,
              () => app.setLlmEnabled(!app.store.llm_enabled),
            )}
            {#if app.store.llm_enabled}
              {@render aiAssistant()}
            {/if}
          </div>
          {/if}
        </section>

        <!-- Paths -->
        <section>
          {@render sectionHeading(
            FolderOpen,
            "Paths",
            sections.paths,
            () => (sections.paths = !sections.paths),
          )}
          {#if sections.paths}
            <div id="drawer-section-paths" class="mt-2 space-y-4">
              <p class="text-[11px] leading-snug text-muted">
                Only needed if protongen can't find things on its own — a Steam install
                somewhere unusual, or tools outside your <code class="font-mono">PATH</code>.
                Changes re-scan automatically.
              </p>

              <!-- What the re-scan found. This is the validation: there is no
                   separate check, so a second implementation can't disagree. -->
              <div class="rounded-lg border border-border bg-surface-2/40 px-2.5 py-2">
                <p class="text-[11px] text-muted">
                  {app.steamRoot ?? "No Steam install found"}
                </p>
                <p class="mt-0.5 text-[11px] text-subtext">
                  {app.runtimes.length} runtimes · {app.games.length} games
                  {#if app.refreshing}<span class="text-muted"> · re-scanning…</span>{/if}
                </p>
              </div>

              {#each app.pathWarnings as w (w.kind + w.path)}
                <p class="text-[11px] leading-snug text-yellow">
                  {w.file} <code class="font-mono">{w.path}</code>: {w.error}
                </p>
              {/each}

              {@render pathList(
                "Steam roots",
                "Tried before the built-in ~/.steam locations.",
                app.store.paths.steam_roots,
                (v) => app.setPathList("steam_roots", v),
              )}
              {@render pathList(
                "Steam libraries",
                "Extra library folders, each containing steamapps/.",
                app.store.paths.steam_libraries,
                (v) => app.setPathList("steam_libraries", v),
              )}
              {@render pathList(
                "Proton directories",
                "A folder holding one sub-folder per Proton build, each with a compatibilitytool.vdf — not a single build.",
                app.store.paths.proton_dirs,
                (v) => app.setPathList("proton_dirs", v),
              )}

              <div>
                <p class="text-[11px] font-medium text-subtext">Program paths</p>
                <p class="mb-1.5 text-[11px] leading-snug text-muted">
                  Emitted into the command as-is. Steam launched from a desktop entry
                  often has a PATH without ~/.local/bin.
                </p>
                {@render binRow("umu-run")}
                {@render binRow("gamescope")}
                {@render binRow("gamemoderun")}
                {@render binRow("mangohud")}
              </div>
            </div>
          {/if}
        </section>
      </div>
            </div>
          </div>
        {/if}
      {/snippet}
    </DialogPrimitive.Content>
  </DialogPrimitive.Portal>
</DialogPrimitive.Root>

<!-- One fix covers all three drawer sections, since they share this snippet. -->
{#snippet sectionHeading(Icon: Component, label: string, isOpen: boolean, onclick: () => void)}
  <button
    {onclick}
    aria-expanded={isOpen}
    aria-controls={`drawer-section-${label.replace(/\W+/g, "-").toLowerCase()}`}
    class="flex w-full items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted transition hover:text-subtext"
  >
    <Icon size={13} />
    {label}
    <CaretDown size={12} class="ml-auto transition-transform {isOpen ? '' : '-rotate-90'}" />
  </button>
{/snippet}

{#snippet pathList(title: string, desc: string, list: string[], onchange: (v: string[]) => void)}
  <div>
    <p class="text-[11px] font-medium text-subtext">{title}</p>
    <p class="mb-1.5 text-[11px] leading-snug text-muted">{desc}</p>
    <div class="space-y-1">
      {#each list as entry, i (i)}
        <div class="flex items-center gap-1">
          <input
            value={entry}
            oninput={(e) => onchange(list.map((x, j) => (j === i ? e.currentTarget.value : x)))}
            aria-label="{title} {i + 1}"
            spellcheck="false"
            class="min-w-0 flex-1 rounded-lg border border-border bg-surface-2/60 px-2 py-1 font-mono text-[11px] text-text"
          />
          <button
            onclick={() => onchange(list.filter((_, j) => j !== i))}
            aria-label="Remove {title} {i + 1}"
            class="grid size-6 shrink-0 place-items-center rounded-lg text-muted transition hover:bg-surface-2 hover:text-red"
          >
            <Trash size={13} />
          </button>
        </div>
      {/each}
      <div class="flex gap-1">
        <button
          onclick={() => onchange([...list, ""])}
          class="rounded-lg border border-border px-2 py-1 text-[11px] text-subtext transition hover:border-accent/50 hover:text-text"
        >
          Add row
        </button>
        {#if inTauri}
          <button
            onclick={async () => {
              const picked = await pickPath({ directory: true, multiple: false });
              if (typeof picked === "string") onchange([...list, picked]);
            }}
            class="rounded-lg border border-border px-2 py-1 text-[11px] text-subtext transition hover:border-accent/50 hover:text-text"
          >
            Browse…
          </button>
        {/if}
      </div>
    </div>
  </div>
{/snippet}

{#snippet binRow(name: string)}
  <div class="flex items-center gap-1.5 py-0.5">
    <code class="w-24 shrink-0 font-mono text-[11px] text-subtext">{name}</code>
    <input
      value={app.store.paths.bins[name] ?? ""}
      oninput={(e) => app.setBinOverride(name, e.currentTarget.value)}
      placeholder="default — found on PATH"
      aria-label="{name} path"
      spellcheck="false"
      class="min-w-0 flex-1 rounded-lg border border-border bg-surface-2/60 px-2 py-1 font-mono text-[11px] text-text"
    />
    {#if inTauri}
      <button
        onclick={async () => {
          const picked = await pickPath({ directory: false, multiple: false });
          if (typeof picked === "string") app.setBinOverride(name, picked);
        }}
        aria-label="Browse for {name}"
        class="grid size-6 shrink-0 place-items-center rounded-lg text-muted transition hover:bg-surface-2 hover:text-text"
      >
        <FolderOpen size={13} />
      </button>
    {/if}
    <Badges requires={name} />
  </div>
{/snippet}

{#snippet toggle(title: string, desc: string, checked: boolean, onchange: () => void)}
  {@const id = `setting-${title.replace(/\W+/g, "-").toLowerCase()}`}
  <div class="flex items-center gap-3 rounded-lg px-1 py-1.5">
    <div class="min-w-0 flex-1">
      <p {id} class="text-sm text-subtext">{title}</p>
      <p class="text-[11px] leading-snug text-muted">{desc}</p>
    </div>
    <Switch {checked} {onchange} labelledby={id} />
  </div>
{/snippet}

<!-- AMD GPU generation selector. Replaces the old "I have an RDNA3/RDNA4 GPU"
     toggle: picking a generation unlocks the FSR 3/4 upgrade options, and then
     filters *both* ways — each generation's exclusive options and recipes hide
     on the other. Only takes effect on a detected AMD GPU (see `hwCaps`). -->
{#snippet gpuGen()}
  {@const gen = app.store.gpu_gen}
  <div class="rounded-lg px-1 py-1.5">
    <p class="text-sm text-subtext">AMD GPU generation</p>
    <p class="mb-1.5 text-[11px] leading-snug text-muted">
      Unlocks FSR 3/4 upscaler-upgrade options (hidden by default), then shows only the
      ones for the generation you pick. Not auto-detected.
    </p>
    <div class="flex gap-1 rounded-lg bg-mantle p-0.5 text-xs">
      {#each GPU_GENS as [value, label] (value)}
        <button
          onclick={() => app.setGpuGen(value)}
          class="flex-1 rounded-md px-2 py-1 font-medium transition"
          class:bg-surface-2={gen === value}
          class:text-text={gen === value}
          class:text-muted={gen !== value}>{label}</button
        >
      {/each}
    </div>
  </div>
{/snippet}

<!-- Global profile: a reusable selection saved from the current build and
     applied to any game on demand via the builder's "Apply global profile"
     button. -->
{#snippet globalProfile()}
  {@const gp = app.store.global_profile}
  <div class="rounded-lg px-1 py-1.5">
    <p class="text-sm text-subtext">Global profile</p>
    <p class="mb-1.5 text-[11px] leading-snug text-muted">
      {#if gp}
        Saved: {gp.env.length} env · {gp.wrappers.length} wrappers{gp.runtime
          ? ` · ${gp.runtime}`
          : ""}. Apply it to any game from the builder.
      {:else}
        Not set. Build a game the way you like, then save it here to reuse elsewhere.
      {/if}
    </p>
    <div class="flex gap-1">
      <button
        onclick={() => app.setGlobalProfileFromCurrent()}
        class="rounded-lg border border-border px-2 py-1 text-[11px] text-subtext transition hover:border-accent/50 hover:text-text"
      >
        Set from current build
      </button>
      {#if gp}
        <button
          onclick={() => app.clearGlobalProfile()}
          class="rounded-lg border border-border px-2 py-1 text-[11px] text-muted transition hover:border-red/50 hover:text-red"
        >
          Clear
        </button>
      {/if}
    </div>
  </div>
{/snippet}

<!-- Local-LLM endpoint + model, shown when the AI coach is enabled. The endpoint
     is the `/v1` base; "Test connection" lists the models the server is serving
     so the user can pick one without typing the exact id. -->
{#snippet aiAssistant()}
  <div class="space-y-2 rounded-lg px-1 py-1.5">
    <div>
      <p class="text-[11px] font-medium text-subtext">Endpoint</p>
      <p class="mb-1.5 text-[11px] leading-snug text-muted">
        The <code class="font-mono">/v1</code> base URL of your local server.
      </p>
      <input
        value={app.store.llm_endpoint}
        oninput={(e) => app.setLlmEndpoint(e.currentTarget.value)}
        placeholder="http://127.0.0.1:1234/v1"
        aria-label="LLM endpoint"
        spellcheck="false"
        class="w-full rounded-lg border border-border bg-surface-2/60 px-2 py-1 font-mono text-[11px] text-text"
      />
    </div>
    <div>
      <p class="text-[11px] font-medium text-subtext">Model</p>
      <div class="flex gap-1">
        <input
          value={app.store.llm_model}
          oninput={(e) => app.setLlmModel(e.currentTarget.value)}
          placeholder="gpt-oss-20b"
          aria-label="LLM model"
          spellcheck="false"
          class="min-w-0 flex-1 rounded-lg border border-border bg-surface-2/60 px-2 py-1 font-mono text-[11px] text-text"
        />
        <button
          onclick={testLlm}
          disabled={llmTest === "loading"}
          class="shrink-0 rounded-lg border border-border px-2 py-1 text-[11px] text-subtext transition hover:border-accent/50 hover:text-text disabled:opacity-60"
        >
          {llmTest === "loading" ? "Testing…" : "Test connection"}
        </button>
      </div>
      {#if llmTest === "ok"}
        <p class="mt-1 text-[11px] text-green">{llmTestMsg}</p>
      {:else if llmTest === "err"}
        <p class="mt-1 text-[11px] leading-snug text-red">Couldn't connect: {llmTestMsg}</p>
      {/if}
      {#if llmModels.length > 0}
        <div class="mt-1.5 flex flex-wrap gap-1">
          {#each llmModels as m (m)}
            <button
              onclick={() => app.setLlmModel(m)}
              class="rounded-lg border px-2 py-0.5 text-[11px] font-mono transition {app.store
                .llm_model === m
                ? 'border-accent text-text'
                : 'border-border text-subtext hover:border-accent/50'}"
            >
              {m}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{/snippet}
