<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { FolderOpen, DownloadSimple } from "phosphor-svelte";
  import Switch from "./Switch.svelte";

  // The dialog plugin only works inside the Tauri shell; in `pnpm dev` (plain
  // browser) it throws, so we hide the Browse button there.
  const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

  let installer = $state(false);

  const exeLabel = $derived(installer ? "Installer .exe (setup.exe)" : "Game .exe");
  const exePlaceholder = $derived(
    installer ? "/path/to/setup.exe" : "/path/to/game.exe",
  );

  async function browse() {
    try {
      const sel = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Windows program", extensions: ["exe", "msi", "bat"] }],
      });
      if (typeof sel === "string") app.umuExe = sel;
    } catch (e) {
      console.error("file dialog failed", e);
    }
  }
</script>

<div class="space-y-3">
  <!-- Installer-mode toggle -->
  <div class="flex items-center gap-3 rounded-lg border border-border/60 bg-surface-2/40 px-3 py-2">
    <div class="min-w-0 flex-1">
      <p class="text-xs font-medium text-subtext">Installer mode</p>
      <p class="text-[11px] leading-snug text-muted">
        Point at a repack's setup.exe to install it; switch off to launch the game.
      </p>
    </div>
    <Switch checked={installer} onchange={() => (installer = !installer)} label="Installer mode" />
  </div>

  <!-- Executable + browse -->
  <label class="block">
    <span class="mb-1 block text-[11px] uppercase tracking-wider text-muted">{exeLabel}</span>
    <div class="flex gap-2">
      <input
        value={app.umuExe}
        oninput={(e) => {
          app.umuExe = e.currentTarget.value;
          app.noteEdit("set the umu executable");
        }}
        placeholder={exePlaceholder}
        class="min-w-0 flex-1 rounded-lg border border-border bg-surface-2 px-2.5 py-2 font-mono text-xs text-text outline-none focus:border-accent"
      />
      {#if isTauri}
        <button
          type="button"
          onclick={browse}
          class="inline-flex shrink-0 items-center gap-1.5 rounded-lg border border-border bg-surface-2/60 px-2.5 py-2 text-xs text-subtext transition hover:border-accent/40 hover:text-text"
        >
          <FolderOpen size={14} /> Browse…
        </button>
      {/if}
    </div>
  </label>

  <!-- WINEPREFIX + GAMEID -->
  <div class="grid grid-cols-1 gap-3 sm:grid-cols-[1fr_9rem]">
    {@render field("WINEPREFIX", "umuWineprefix", "(optional) /path/to/prefix")}
    {@render field("GAMEID", "umuGameid", "umu-0")}
  </div>
</div>

<p class="mt-2 flex items-start gap-1.5 text-xs text-muted">
  {#if app.selectedRuntime?.kind === "auto"}
    <DownloadSimple size={14} class="mt-0.5 shrink-0 text-accent" />
  {/if}
  <span>
    PROTONPATH comes from the Proton runtime above.{#if installer}
      To install a repack: pick its <span class="font-mono">setup.exe</span> and a GAMEID,
      copy &amp; run the command once, then turn off Installer mode and point at the game's
      <span class="font-mono">.exe</span> with the <strong>same GAMEID</strong>, so umu reuses
      <span class="font-mono">~/Games/umu/&lt;GAMEID&gt;</span> as the prefix.{/if}
  </span>
</p>

{#snippet field(label: string, key: "umuWineprefix" | "umuGameid", placeholder: string)}
  <label class="block">
    <span class="mb-1 block text-[11px] uppercase tracking-wider text-muted">{label}</span>
    <input
      value={app[key]}
      oninput={(e) => {
        app[key] = e.currentTarget.value;
        app.noteEdit(`set ${label}`);
      }}
      {placeholder}
      class="w-full rounded-lg border border-border bg-surface-2 px-2.5 py-2 font-mono text-xs text-text outline-none focus:border-accent"
    />
  </label>
{/snippet}
