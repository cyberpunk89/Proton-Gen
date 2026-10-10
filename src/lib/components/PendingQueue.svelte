<script lang="ts">
  import { untrack } from "svelte";
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import { inTauri } from "$lib/ipc";
  import { copyText, openSteamUrl, steamPropertiesUrl } from "$lib/util";
  import Dialog from "./Dialog.svelte";
  import { CheckCircle, CircleDashed, WarningCircle, SteamLogo, ArrowSquareOut } from "phosphor-svelte";

  /**
   * "Pending in Steam": every tuned game whose saved options aren't what
   * Steam has, walked one at a time. Each row copies that game's command
   * (built from its saved tuning, no need to open it) and opens its Steam
   * Properties; the row ticks itself off when the watcher / focus re-read sees
   * Steam's config catch up, and the next unticked row becomes the primary.
   *
   * The list is frozen when the dialog opens, so a game that goes green stays
   * on screen as done instead of vanishing out from under the pointer.
   */
  let ids = $state<number[]>([]);
  let opened = $state<Record<number, boolean>>({});
  let busy = $state<number | null>(null);

  $effect(() => {
    if (!app.pendingQueueOpen) return;
    untrack(() => {
      ids = app.pendingInSteam.map((g) => g.app_id);
      opened = {};
    });
  });

  let rows = $derived(
    ids
      .map((id) => app.games.find((g) => g.app_id === id))
      .filter((g) => g != null)
      .map((g) => ({
        game: g,
        status: app.launchStatuses[String(g.app_id)],
        proton: app.savedRuntimeMismatch(g.app_id),
      })),
  );
  let isDone = (r: (typeof rows)[number]) => r.status === "in-sync";
  let doneCount = $derived(rows.filter(isDone).length);
  let current = $derived(rows.find((r) => !isDone(r))?.game.app_id ?? null);

  async function copyAndOpen(appId: number) {
    busy = appId;
    try {
      await copyText(await app.commandForSaved(appId));
    } catch (e) {
      toast.error(`Couldn't copy the command: ${e instanceof Error ? e.message : e}`);
      busy = null;
      return;
    }
    app.expectPaste();
    opened[appId] = true;
    busy = null;
    if (!(await openSteamUrl(steamPropertiesUrl(appId)))) {
      toast.info(
        inTauri
          ? "Copied — but couldn't hand the link to Steam. Is Steam installed?"
          : "Copied. Steam deep links only work in the desktop app.",
      );
    }
  }

  function openInBuilder(appId: number) {
    app.pendingQueueOpen = false;
    // After the dialog's close, so two modal layers never overlap (#63).
    setTimeout(() => app.openRequestedGame(appId), 0);
  }
</script>

<Dialog
  bind:open={app.pendingQueueOpen}
  title="Pending in Steam"
  subtitle="Tuned games whose launch options aren't in Steam yet."
  width="38rem"
>
  {#if !rows.length}
    <p class="py-6 text-center text-sm text-muted">Everything you've tuned is in Steam.</p>
  {:else}
    <p class="mb-3 text-xs text-muted" aria-live="polite">
      {doneCount} of {rows.length} done. For each game: copy &amp; open, paste into
      <span class="text-subtext">Launch Options</span> on the General tab, close — the row ticks
      itself off when Steam saves.
    </p>
    <ul class="max-h-[55vh] space-y-1.5 overflow-y-auto">
      {#each rows as r (r.game.app_id)}
        {@const done = isDone(r)}
        {@const primary = r.game.app_id === current}
        <li
          class="flex items-center gap-3 rounded-lg border px-3 py-2 text-sm {primary
            ? 'border-accent/50 bg-accent/5'
            : 'border-border/60'}"
        >
          {#if done}
            <CheckCircle size={16} weight="fill" class="shrink-0 text-green" />
          {:else if r.status === "drifted"}
            <WarningCircle size={16} weight="fill" class="shrink-0 text-peach" />
          {:else}
            <CircleDashed size={16} weight="bold" class="shrink-0 text-accent" />
          {/if}
          <div class="min-w-0 flex-1">
            <p class="truncate text-text">{r.game.name}</p>
            <p class="truncate text-xs text-muted">
              {#if done}
                In Steam
              {:else}
                {r.status === "drifted" ? "Changes not pasted" : "Not in Steam yet"}{opened[r.game.app_id]
                  ? " · copied, waiting for Steam"
                  : ""}
              {/if}
              {#if r.proton && !done}
                · then set Proton to <span class="text-subtext">{r.proton}</span>
              {/if}
            </p>
          </div>
          <button
            onclick={() => openInBuilder(r.game.app_id)}
            class="shrink-0 rounded p-1 text-muted transition hover:text-text"
            title="Open in the builder"
            aria-label="Open {r.game.name} in the builder"
          >
            <ArrowSquareOut size={14} />
          </button>
          {#if !done}
            <button
              onclick={() => copyAndOpen(r.game.app_id)}
              disabled={busy === r.game.app_id}
              class="inline-flex shrink-0 items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs font-medium transition active:scale-95 disabled:opacity-50 {primary
                ? ''
                : 'border border-border bg-surface-2/50 text-subtext hover:border-accent/50'}"
              style={primary ? "background: var(--accent); color: var(--on-accent)" : undefined}
            >
              <SteamLogo size={13} weight="bold" /> Copy &amp; open
            </button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</Dialog>
