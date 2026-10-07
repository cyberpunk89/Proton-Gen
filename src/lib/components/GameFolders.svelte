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
  let seq = 0;

  $effect(() => {
    const id = app.selectedAppId;
    folders = null;
    if (id == null) return;
    // Measured once per game per session: a prefix walk is real disk work,
    // and switching back and forth between games shouldn't repeat it.
    const hit = measured.get(id);
    if (hit) {
      folders = hit;
      return;
    }
    const mine = ++seq;
    ipc
      .gameFolders(id)
      .then((f) => {
        measured.set(id, f);
        if (mine === seq) folders = f;
      })
      .catch(() => mine === seq && (folders = []));
  });

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

{#if folders === null && app.selectedAppId != null}
  <p class="text-xs text-muted" aria-live="polite">Measuring prefix…</p>
{:else if folders && folders.length}
  <div class="space-y-1.5 rounded-xl border border-border/60 bg-surface-2/40 p-3">
    <p class="text-[11px] font-medium uppercase tracking-wider text-muted">Prefix &amp; caches</p>
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
