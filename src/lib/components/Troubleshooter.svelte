<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import Dialog from "./Dialog.svelte";
  import RecipePreview from "./RecipePreview.svelte";
  import { Robot, Lightning, Wrench, ArrowsClockwise } from "phosphor-svelte";

  /**
   * The AI symptom troubleshooter, mounted once at the app root and opened from
   * the header (in both Simple and Advanced). The user describes a problem in
   * their own words; the backend diagnoses it, recommending existing Fix recipes
   * where they fit and proposing catalog changes otherwise. The current game's
   * log is pulled in as optional context by `app.troubleshoot`.
   */
  let symptom = $state("");

  // Reset the result whenever the dialog closes, so reopening starts clean and a
  // stale diagnosis never shows against a different game. Only reads the open
  // flag; the clear writes result/error/loading, never the flag, so no loop.
  $effect(() => {
    if (!app.showTroubleshooter) app.clearTroubleshoot();
  });

  function submit() {
    const s = symptom.trim();
    if (!s || app.tsLoading) return;
    void app.troubleshoot(s);
  }

  function onkeydown(e: KeyboardEvent) {
    // Ctrl/Cmd+Enter submits from the textarea.
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      submit();
    }
  }

  async function applyRecipe(index: number, name: string) {
    await app.applyRecipe(index);
    toast.success(`Applied: ${name}`);
  }

  function applyChange(change: { key: string; value: string }) {
    if (app.applyLlmChange(change)) {
      toast.success(`Applied ${change.key}${change.value ? `=${change.value}` : ""}`);
    }
  }

  const EXAMPLES = [
    "Black screen on launch",
    "Crashes after the intro video",
    "Stutters badly every few seconds",
    "No audio in-game",
  ];
</script>

<Dialog
  bind:open={app.showTroubleshooter}
  title="AI troubleshooter"
  subtitle={app.selectedGameName
    ? `Describe the problem with ${app.selectedGameName}`
    : "Describe the problem you're seeing"}
  width="52rem"
>
  <div class="space-y-3">
    <!-- Symptom input -->
    <div>
      <textarea
        bind:value={symptom}
        {onkeydown}
        rows="2"
        placeholder="e.g. the game shows a black screen on launch, then closes"
        aria-label="Describe the problem"
        class="w-full resize-y rounded-lg border border-border bg-surface-2/60 px-3 py-2 text-sm text-text placeholder:text-muted focus:border-accent/60 focus:outline-none"
      ></textarea>
      <div class="mt-1.5 flex flex-wrap items-center gap-1.5">
        {#each EXAMPLES as ex (ex)}
          <button
            onclick={() => (symptom = ex)}
            class="rounded-full border border-border px-2 py-0.5 text-[11px] text-muted transition hover:border-accent/50 hover:text-subtext"
          >
            {ex}
          </button>
        {/each}
        <button
          onclick={submit}
          disabled={app.tsLoading || !symptom.trim()}
          class="ml-auto inline-flex items-center gap-1.5 rounded-lg border border-accent/40 bg-accent/10 px-3 py-1.5 text-xs font-medium text-accent transition hover:bg-accent/20 disabled:opacity-60"
        >
          <Robot size={13} class={app.tsLoading ? "animate-pulse" : ""} />
          {app.tsLoading ? "Diagnosing…" : "Diagnose"}
        </button>
      </div>
    </div>

    {#if app.tsLoading}
      <p class="py-6 text-center text-sm text-muted">Reading the problem and thinking… (local model)</p>
    {:else if app.tsError}
      <div class="rounded-lg border border-red/40 bg-red/5 p-3">
        <p class="text-sm text-red">Couldn't reach the AI: {app.tsError}</p>
        <p class="mt-1 text-xs text-muted">
          Check the endpoint in Settings and that your local server has a model loaded.
        </p>
        <button
          onclick={submit}
          class="mt-2 inline-flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1 text-xs text-subtext transition hover:border-accent/50"
        >
          <ArrowsClockwise size={12} /> Retry
        </button>
      </div>
    {:else if app.tsResult}
      <!-- Diagnosis -->
      <div class="rounded-lg border border-accent/30 bg-accent/5 p-3">
        <p class="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-accent">
          <Robot size={14} weight="fill" /> Diagnosis
          <span class="font-normal text-muted">· {app.store.llm_model}</span>
        </p>
        <p class="whitespace-pre-wrap text-sm leading-relaxed text-subtext">{app.tsResult.text}</p>
      </div>

      <!-- Recommended Fix recipes -->
      {#if app.tsResult.recipes.length > 0}
        <div>
          <p class="mb-1.5 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted">
            <Wrench size={13} /> Recommended fixes
          </p>
          <div class="space-y-2">
            {#each app.tsResult.recipes as idx (idx)}
              {@const r = app.recipes[idx]}
              {#if r}
                {@const accent = r.accent ?? "var(--accent)"}
                <div
                  class="flex flex-col gap-2 rounded-xl border border-border/70 bg-surface-solid/40 p-3"
                  style="--rc: {accent}"
                >
                  <div class="min-w-0">
                    <p class="text-sm font-medium text-text">{r.name}</p>
                    {#if r.symptom}
                      <p class="text-xs italic text-muted">{r.symptom}</p>
                    {/if}
                    {#if r.description}
                      <p class="mt-1 text-xs leading-snug text-subtext">{r.description}</p>
                    {/if}
                  </div>
                  <div class="flex items-center gap-1.5">
                    <RecipePreview index={idx} {accent} />
                    <button
                      onclick={() => applyRecipe(idx, r.name)}
                      class="ml-auto rounded-lg px-2.5 py-1 text-xs font-medium transition active:scale-95"
                      style="background: color-mix(in srgb, {accent} 20%, transparent); color: {accent}"
                    >
                      Apply
                    </button>
                  </div>
                </div>
              {/if}
            {/each}
          </div>
        </div>
      {/if}

      <!-- Extra catalog changes not covered by a recipe -->
      {#if app.tsResult.changes.length > 0}
        <div>
          <p class="mb-1.5 flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wider text-muted">
            <Lightning size={13} /> Other changes to try
          </p>
          <div class="flex flex-wrap gap-1.5">
            {#each app.tsResult.changes as c (c.key + c.value)}
              {#if app.hasCatalogKey(c.key)}
                <button
                  onclick={() => applyChange(c)}
                  title={c.reason}
                  class="inline-flex items-center gap-1.5 rounded-lg border border-accent/40 bg-accent/10 px-2.5 py-1 text-xs font-medium text-accent transition hover:bg-accent/20"
                >
                  <Lightning size={12} weight="fill" /> Apply {c.key}{c.value ? `=${c.value}` : ""}
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
        </div>
      {/if}

      {#if app.tsResult.recipes.length === 0 && app.tsResult.changes.length === 0}
        <p class="text-xs text-muted">
          No concrete recipe or catalog change to suggest — see the diagnosis above.
        </p>
      {/if}
    {:else}
      <p class="py-6 text-center text-sm text-muted">
        Describe what's going wrong and the local AI will suggest fixes — recommending a
        one-click recipe when one fits.
      </p>
    {/if}
  </div>
</Dialog>
