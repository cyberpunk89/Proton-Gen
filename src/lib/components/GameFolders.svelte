<script lang="ts" module>
  import type { GameFolder as Measured } from "$lib/types";
  /** Session cache shared by every mount: appid -> measured folders. */
  const measured = new Map<number, Measured[]>();
</script>

<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { ipc } from "$lib/ipc";
  import { toast } from "$lib/toast.svelte";
  import type { GameFolder } from "$lib/types";
  import { FolderOpen } from "phosphor-svelte";

  /**
   * The selected game's Wine prefix and shader cache: size, and a button to
   * open each in the file manager. Read-only. Measured per game on open (a
   * prefix walk is tens of thousands of files, so never for the whole
   * library); the latest request wins if the user switches games mid-walk.
   */
  let folders = $state<GameFolder[] | null>(null);
  let measuring = $state(false);
  let seq = 0;

  // Listing the folders is a few stats; sizes are a walk of the whole prefix,
  // so they're measured only on request, then remembered for the session.
  $effect(() => {
    const id = app.selectedAppId;
    folders = null;
    measuring = false;
    if (id == null) return;
    const hit = measured.get(id);
    if (hit) {
      folders = hit;
      return;
    }
    const mine = ++seq;
    ipc
      .gameFolders(id, false)
      .then((f) => mine === seq && (folders = f))
      .catch(() => mine === seq && (folders = []));
  });

  async function measure() {
    const id = app.selectedAppId;
    if (id == null || measuring) return;
    measuring = true;
    const mine = ++seq;
    try {
      const f = await ipc.gameFolders(id, true);
      measured.set(id, f);
      if (mine === seq) folders = f;
    } catch (e) {
      toast.error(`Couldn't measure: ${e}`);
    } finally {
      if (mine === seq) measuring = false;
    }
  }

  let unmeasured = $derived(!!folders?.some((f) => f.exists && f.bytes == null));

  const LABEL: Record<GameFolder["kind"], string> = {
    prefix: "Wine prefix",
    shadercache: "Shader cache",
  };

  function human(bytes: number | null): string {
    if (bytes == null) return "";
    if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
    if (bytes < 1024 ** 3) return `${Math.round(bytes / 1024 ** 2)} MB`;
    return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
  }

  async function open(f: GameFolder) {
    if (app.selectedAppId == null) return;
    try {
      await ipc.openGameFolder(app.selectedAppId, f.kind);
    } catch (e) {
      toast.error(`Couldn't open it: ${e}`);
    }
  }
</script>

{#if folders && folders.length}
  <div class="space-y-1.5 rounded-xl border border-border/60 bg-surface-2/40 p-3">
    <div class="flex items-center justify-between gap-2">
      <p class="text-[11px] font-medium uppercase tracking-wider text-muted">Prefix &amp; caches</p>
      {#if unmeasured}
        <button
          onclick={measure}
          disabled={measuring}
          aria-live="polite"
          class="text-[11px] text-accent transition hover:opacity-80 disabled:opacity-60"
          >{measuring ? "Measuring…" : "Show sizes"}</button
        >
      {/if}
    </div>
    {#if app.selectedGame?.size_on_disk}
      <!-- From the appmanifest, so free — no folder walk behind "Show sizes". -->
      <div class="flex items-center gap-2 text-xs">
        <span class="w-24 shrink-0 text-subtext">Install</span>
        <span class="min-w-0 flex-1 truncate font-mono text-[11px] text-muted" title={app.selectedGame.install_dir ?? ""}
          >{app.selectedGame.install_dir ?? ""}</span
        >
        <span class="shrink-0 font-mono text-[11px] text-subtext">{human(app.selectedGame.size_on_disk)}</span>
      </div>
    {/if}
    {#each folders as f (f.kind)}
      <div class="flex items-center gap-2 text-xs">
        <span class="w-24 shrink-0 text-subtext">{LABEL[f.kind]}</span>
        <span class="min-w-0 flex-1 truncate font-mono text-[11px] text-muted" title={f.path}>{f.path}</span>
        {#if f.exists}
          <span class="shrink-0 font-mono text-[11px] text-subtext">{human(f.bytes)}</span>
          <button
            onclick={() => open(f)}
            class="inline-flex shrink-0 items-center gap-1 rounded-md border border-border px-1.5 py-0.5 text-[11px] text-subtext transition hover:border-accent/50 hover:text-text"
            aria-label="Open {LABEL[f.kind].toLowerCase()} folder"
          >
            <FolderOpen size={12} /> Open
          </button>
        {:else}
          <span class="shrink-0 text-[11px] text-muted">not created yet</span>
        {/if}
      </div>
    {/each}
  </div>
{/if}
