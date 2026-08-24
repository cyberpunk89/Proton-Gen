<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { ipc } from "$lib/ipc";
  import { toast } from "$lib/toast.svelte";
  import Dialog from "./Dialog.svelte";
  import type { ProtonLog } from "$lib/types";
  import {
    ArrowsClockwise,
    WarningCircle,
    FileText,
    Sparkle,
    Robot,
    Lightning,
  } from "phosphor-svelte";

  /**
   * The per-game Proton log viewer, mounted once at the app root and opened from
   * the header for the selected game. `PROTON_LOG=1` writes ~/steam-<appid>.log;
   * this reads its tail (read-only) so a stubborn game can be diagnosed without
   * leaving the app.
   */
  let log = $state<ProtonLog | null>(null);
  let loading = $state(false);
  let loadError = $state<string | null>(null);

  async function refresh() {
    const id = app.selectedAppId;
    if (id == null) return;
    loading = true;
    loadError = null;
    // A fresh log means the previous analysis no longer applies.
    app.clearAnalysis();
    try {
      log = await ipc.readProtonLog(id);
    } catch (e) {
      loadError = String(e);
      log = null;
    } finally {
      loading = false;
    }
  }

  function analyze() {
    if (!log?.present) return;
    void app.analyzeLog({ error_lines: log.error_lines, tail: log.tail });
  }

  function apply(change: { key: string; value: string }) {
    if (app.applyLlmChange(change)) {
      toast.success(`Applied ${change.key}${change.value ? `=${change.value}` : ""}`);
    }
  }

  // Fetch when the dialog opens (or the selected game changes while it's open).
  // Reads only showLogs + selectedAppId, so writing log/loading here can't loop.
  $effect(() => {
    if (app.showLogs && app.selectedAppId != null) void refresh();
  });

  const loggingOn = $derived(app.env["PROTON_LOG"]?.enabled ?? false);

  function enableLogging() {
    const cur = app.env["PROTON_LOG"];
    if (!cur) {
      toast.error("PROTON_LOG isn't in the catalog");
      return;
    }
    if (!cur.enabled) app.toggleEnv("PROTON_LOG");
    app.setEnvValue("PROTON_LOG", "1");
    toast.success("Logging enabled — apply the command and relaunch the game", { ms: 6000 });
  }

  function human(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

<Dialog
  bind:open={app.showLogs}
  title="Proton log"
  subtitle={app.selectedGameName
    ? `Diagnostics for ${app.selectedGameName}`
    : "Diagnostics for the selected game"}
  width="52rem"
>
  <div class="space-y-3">
    <!-- Path + actions -->
    <div class="flex flex-wrap items-center gap-2">
      <span
        class="inline-flex min-w-0 flex-1 items-center gap-1.5 truncate rounded-lg border border-border bg-surface-2/60 px-2.5 py-1.5 font-mono text-[11px] text-subtext"
      >
        <FileText size={13} class="shrink-0 text-muted" />
        <span class="truncate">{log?.path ?? "…"}</span>
        {#if log?.present}
          <span class="shrink-0 text-muted">· {human(log.size)}</span>
        {/if}
      </span>
      <button
        onclick={refresh}
        disabled={loading || app.selectedAppId == null}
        class="inline-flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50 disabled:opacity-60"
      >
        <ArrowsClockwise size={13} class={loading ? "animate-spin" : ""} /> Refresh
      </button>
      {#if !loggingOn}
        <button
          onclick={enableLogging}
          class="inline-flex items-center gap-1.5 rounded-lg border border-accent/40 px-2.5 py-1.5 text-xs font-medium text-accent transition hover:bg-accent/10"
        >
          <Sparkle size={13} /> Enable logging
        </button>
      {/if}
      {#if app.store.llm_enabled}
        <button
          onclick={analyze}
          disabled={app.aiLoading || !log?.present}
          class="inline-flex items-center gap-1.5 rounded-lg border border-accent/40 px-2.5 py-1.5 text-xs font-medium text-accent transition hover:bg-accent/10 disabled:opacity-60"
          title="Send this log to your local AI for tuning suggestions"
        >
          <Robot size={13} class={app.aiLoading ? "animate-pulse" : ""} />
          {app.aiLoading ? "Analyzing…" : "Analyze with AI"}
        </button>
      {/if}
    </div>

    {#if loadError}
      <p class="rounded-lg border border-red/40 bg-red/5 px-3 py-2 text-sm text-red">
        Couldn't read the log: {loadError}
      </p>
    {:else if log && !log.present}
      <div class="flex flex-col items-center gap-2 py-10 text-center">
        <FileText size={26} class="text-muted" />
        <p class="text-sm text-muted">No log yet for this game.</p>
        <p class="max-w-md text-xs text-muted">
          Turn on <span class="font-mono text-subtext">PROTON_LOG</span>, apply the command to
          Steam, then relaunch the game once — the log appears here afterwards.
        </p>
      </div>
    {:else if log && log.present}
      {#if log.error_lines.length > 0}
        <div class="rounded-lg border border-yellow/40 bg-yellow/5 p-2.5">
          <p class="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-yellow">
            <WarningCircle size={14} weight="fill" />
            {log.error_lines.length} line{log.error_lines.length === 1 ? "" : "s"} worth a look
          </p>
          <div class="max-h-40 space-y-0.5 overflow-y-auto">
            {#each log.error_lines as line, i (i)}
              <p class="whitespace-pre-wrap break-all font-mono text-[11px] text-subtext">{line}</p>
            {/each}
          </div>
        </div>
      {/if}

      <!-- AI coach: analysis + one-click apply chips for catalog-backed changes -->
      {#if app.store.llm_enabled && (app.aiLoading || app.aiError || app.aiResult)}
        <div class="rounded-lg border border-accent/30 bg-accent/5 p-3">
          <p class="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-accent">
            <Robot size={14} weight="fill" /> AI suggestions
            <span class="font-normal text-muted">· {app.store.llm_model}</span>
          </p>
          {#if app.aiLoading}
            <p class="text-sm text-muted">Reading the log and thinking… (local model)</p>
          {:else if app.aiError}
            <p class="text-sm text-red">
              Couldn't reach the AI: {app.aiError}
            </p>
            <p class="mt-1 text-xs text-muted">
              Check the endpoint in Settings and that your local server has a model loaded.
            </p>
            <button
              onclick={analyze}
              class="mt-2 inline-flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1 text-xs text-subtext transition hover:border-accent/50"
            >
              <ArrowsClockwise size={12} /> Retry
            </button>
          {:else if app.aiResult}
            <p class="whitespace-pre-wrap text-sm leading-relaxed text-subtext">
              {app.aiResult.text}
            </p>
            {#if app.aiResult.changes.length > 0}
              <div class="mt-2.5 flex flex-wrap gap-1.5">
                {#each app.aiResult.changes as c (c.key + c.value)}
                  {#if app.hasCatalogKey(c.key)}
                    <button
                      onclick={() => apply(c)}
                      title={c.reason}
                      class="inline-flex items-center gap-1.5 rounded-lg border border-accent/40 bg-accent/10 px-2.5 py-1 text-xs font-medium text-accent transition hover:bg-accent/20"
                    >
                      <Lightning size={12} weight="fill" /> Apply {c.key}{c.value
                        ? `=${c.value}`
                        : ""}
                    </button>
                  {:else}
                    <span
                      title={c.reason}
                      class="inline-flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1 text-xs text-muted"
                    >
                      {c.key}{c.value ? `=${c.value}` : ""}
                    </span>
                  {/if}
                {/each}
              </div>
            {/if}
          {/if}
        </div>
      {/if}

      <div>
        <p class="mb-1 text-[11px] uppercase tracking-wider text-muted">
          {log.truncated ? "Tail of log (head trimmed)" : "Log"}
        </p>
        <pre
          class="max-h-[46vh] overflow-auto rounded-lg border border-border bg-mantle/40 p-3 font-mono text-[11px] leading-relaxed text-subtext">{log.tail}</pre>
      </div>
    {:else}
      <p class="py-10 text-center text-sm text-muted">Reading log…</p>
    {/if}
  </div>
</Dialog>
