<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toast } from "$lib/toast.svelte";
  import { copyText, irrelevance, splitExtraEnv } from "$lib/util";
  import Badges from "./Badges.svelte";
  import { ListChecks, X, Copy, Sparkle, Warning, ArrowRight } from "phosphor-svelte";

  /**
   * "What have I actually turned on?" — with 87 parameters across 11 categories
   * that question previously required clicking through every category.
   *
   * Derived from the catalog plus app.env/app.wrap rather than from app.command,
   * because we need the defs: to remove precisely, to show relevance badges, and
   * to warn about options enabled on hardware that can't use them.
   */

  let wrappers = $derived(app.catalog.wrappers.filter((w) => app.wrap[w.key]?.enabled));

  /** Enabled env vars, sub-grouped by catalog category and in catalog order. */
  let envGroups = $derived.by(() => {
    const by = new Map<string, typeof app.catalog.envs>();
    for (const e of app.catalog.envs) {
      if (!app.env[e.key]?.enabled) continue;
      const list = by.get(e.category);
      if (list) list.push(e);
      else by.set(e.category, [e]);
    }
    return [...by.entries()].map(([name, items]) => ({ name, items }));
  });

  let custom = $derived(splitExtraEnv(app.extraEnv));

  /** The warning that exists nowhere else in the app: an option you enabled that
   *  your hardware cannot use. Everywhere else such options are simply hidden. */
  function warn(it: { gpu: string | null; needs: string[] }): string | null {
    return irrelevance(app.hwCaps, it.gpu, it.needs);
  }

  function removeEnv(key: string) {
    app.toggleEnv(key);
    toast.success(`Removed ${key}`, { action: { label: "Undo", onClick: () => app.undo() } });
  }

  function removeWrap(key: string) {
    app.toggleWrap(key);
    toast.success(`Removed ${key}`, { action: { label: "Undo", onClick: () => app.undo() } });
  }

  function removeCustom(raw: string) {
    app.removeExtraEnv(raw);
    toast.success(`Removed ${raw.split("=")[0]}`, {
      action: { label: "Undo", onClick: () => app.undo() },
    });
  }

  function clearAll() {
    app.resetCommand();
    toast.success("Cleared all options", {
      action: { label: "Undo", onClick: () => app.undo() },
    });
  }

  async function copy() {
    await copyText(app.command);
    toast.success("Command copied");
  }
</script>

<section class="card p-4">
  <div class="mb-3 flex flex-wrap items-center gap-2.5">
    <ListChecks size={18} class="text-accent" />
    <h2 class="text-sm font-medium tracking-wide text-text">
      {app.activeCount} option{app.activeCount === 1 ? "" : "s"} active
    </h2>
    {#if app.activeCount > 0}
      <div class="ml-auto flex items-center gap-1.5">
        <button
          onclick={copy}
          class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/50 px-2.5 py-1 text-xs text-subtext transition hover:border-accent/50"
        >
          <Copy size={13} /> Copy
        </button>
        <button
          onclick={clearAll}
          class="rounded-lg border border-border bg-surface-2/50 px-2.5 py-1 text-xs text-subtext transition hover:border-red/50 hover:text-red"
        >
          Clear all
        </button>
      </div>
    {/if}
  </div>

  {#if app.activeCount === 0}
    <div class="flex flex-col items-center gap-2 py-12 text-center">
      <ListChecks size={26} class="text-muted" />
      <p class="text-sm text-muted">Nothing enabled yet.</p>
      <button
        onclick={() => app.setSection("recipes")}
        class="inline-flex items-center gap-1 text-xs text-accent hover:underline"
      >
        <Sparkle size={13} /> Browse recipes
      </button>
    </div>
  {:else}
    <div class="space-y-4">
      {#if wrappers.length}
        <div>
          <p class="mb-1 text-[11px] font-medium uppercase tracking-wider text-muted">Wrappers</p>
          <div class="space-y-0.5">
            {#each wrappers as w (w.key)}
              {@render line(
                w.label ?? w.key,
                app.wrap[w.key]?.value ?? "",
                warn(w),
                w,
                () => removeWrap(w.key),
                false,
              )}
            {/each}
          </div>
        </div>
      {/if}

      {#each envGroups as g (g.name)}
        <div>
          <p class="mb-1 text-[11px] font-medium uppercase tracking-wider text-muted">{g.name}</p>
          <div class="space-y-0.5">
            {#each g.items as e (e.key)}
              {@render line(e.key, app.env[e.key]?.value ?? "", warn(e), e, () => removeEnv(e.key), true)}
            {/each}
          </div>
        </div>
      {/each}

      {#if custom.length}
        <div>
          <p class="mb-1 text-[11px] font-medium uppercase tracking-wider text-muted">Custom env</p>
          <div class="space-y-0.5">
            {#each custom as c (c.raw)}
              {@render line(c.key, c.value, null, null, () => removeCustom(c.raw), true)}
            {/each}
          </div>
        </div>
      {/if}

      <!-- Read-only: these belong to the game/runtime panel, and duplicating the
           controls here would give two places to change one thing. -->
      <div>
        <p class="mb-1 text-[11px] font-medium uppercase tracking-wider text-muted">
          Runtime &amp; mode
        </p>
        <button
          onclick={() => app.setSection("game")}
          class="flex w-full items-center gap-2 rounded-xl px-3 py-2 text-left text-xs transition hover:bg-surface-2/40"
        >
          <span class="text-subtext">
            {app.umu ? "umu-launcher" : "Steam"} ·
            <span class="font-mono">{app.selectedRuntime?.display_name ?? "no runtime"}</span>
            {#if app.gameArgs.trim()}
              · <span class="font-mono">{app.gameArgs.trim()}</span>
            {/if}
          </span>
          <ArrowRight size={12} class="ml-auto shrink-0 text-muted" />
        </button>
      </div>
    </div>
  {/if}
</section>

{#snippet line(
  label: string,
  value: string,
  warning: string | null,
  def: { requires: string | null; gpu: string | null; needs: string[] } | null,
  remove: () => void,
  mono: boolean,
)}
  <div class="flex items-center gap-2 rounded-xl px-3 py-1.5 hover:bg-surface-2/40">
    <span class="min-w-0 flex-1 truncate {mono ? 'font-mono text-[13px]' : 'text-sm'} text-text">
      {label}{#if value}<span class="text-muted">={value}</span>{/if}
    </span>

    {#if warning}
      <span class="inline-flex shrink-0 items-center gap-1 text-[11px] text-peach" title={warning}>
        <Warning size={12} weight="fill" /> {warning}
      </span>
    {/if}

    {#if def}
      <Badges requires={def.requires} gpu={def.gpu} needs={def.needs} />
    {/if}

    <button
      onclick={remove}
      aria-label="Remove {label}"
      title="Remove {label}"
      class="grid size-6 shrink-0 place-items-center rounded-md text-muted transition hover:bg-surface-2 hover:text-red"
    >
      <X size={13} weight="bold" />
    </button>
  </div>
{/snippet}
