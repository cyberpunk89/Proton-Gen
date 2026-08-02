<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { ipc } from "$lib/ipc";
  import Popover from "./Popover.svelte";
  import { mergeStyle } from "$lib/util";
  import type { RecipeChange } from "$lib/types";

  /**
   * "What will Apply actually do?" — answered before you click it.
   *
   * recipes::apply is additive-only: it enables and sets its listed keys and
   * never disables anything, so stacking recipes silently accumulates state.
   * This makes that visible; it does not change the merge semantics.
   *
   * Deliberately not a confirmation dialog in front of Apply — that would break
   * the one-click promise the card makes. It is a chip you can ignore.
   */
  let { index, accent }: { index: number; accent: string } = $props();

  let changes = $state<RecipeChange[] | null>(null);
  let loading = $state(false);

  /**
   * Fetched lazily on hover/focus, and re-fetched every time. No caching: the
   * config changes constantly, so a config-keyed cache would thrash and a
   * config-blind one would go stale — which is worse than a round trip, because
   * a stale preview of a destructive-ish action is a lie.
   */
  async function load() {
    if (loading) return;
    loading = true;
    try {
      changes = await ipc.previewRecipe(index, app.toConfig());
    } catch (e) {
      console.error("previewRecipe failed", e);
      changes = null;
    } finally {
      loading = false;
    }
  }

  let enables = $derived(changes?.filter((c) => c.kind === "enable").length ?? 0);
  let valueChanges = $derived(changes?.filter((c) => c.kind === "value_change").length ?? 0);
  let extras = $derived(changes?.filter((c) => c.kind === "extra_env").length ?? 0);
  let noops = $derived(changes?.filter((c) => c.kind === "no_op").length ?? 0);
  let touches = $derived(enables + valueChanges + extras);
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<span onmouseenter={load} onfocusin={load}>
  <Popover align="start" width="18rem">
    {#snippet trigger({ props })}
      <button
        {...props}
        type="button"
        class="rounded-full px-1.5 py-0.5 font-mono text-[10px] transition hover:brightness-125"
        style={mergeStyle(
          props,
          `background: color-mix(in srgb, ${accent} 14%, transparent)`,
          `color: ${accent}`,
        )}
        aria-label="Preview what this recipe changes"
      >
        {#if changes === null}
          ⋯
        {:else if touches === 0}
          no change
        {:else}
          +{enables}{valueChanges ? ` · ~${valueChanges}` : ""}{extras ? ` · ⤷${extras}` : ""}
        {/if}
      </button>
    {/snippet}

    <div class="space-y-2 text-xs">
      {#if changes === null}
        <p class="text-muted">Loading…</p>
      {:else if changes.length === 0}
        <p class="text-muted">This recipe sets nothing.</p>
      {:else}
        {#if touches === 0}
          <p class="text-muted">
            Everything here is already set — applying would change nothing.
          </p>
        {/if}
        <ul class="space-y-1 font-mono text-[11px]">
          {#each changes as c (c.key)}
            <li class="flex items-start gap-1.5">
              {#if c.kind === "enable"}
                <span class="text-green">+</span>
                <span class="min-w-0 flex-1 break-all text-subtext"
                  >{c.key}{c.to ? `=${c.to}` : ""}</span
                >
              {:else if c.kind === "value_change"}
                <span class="text-peach">~</span>
                <span class="min-w-0 flex-1 break-all text-subtext">
                  {c.key}: <span class="text-muted">{c.from}</span> → {c.to}
                </span>
              {:else if c.kind === "extra_env"}
                <span class="text-blue">⤷</span>
                <span class="min-w-0 flex-1 break-all text-subtext"
                  >{c.key}={c.to} <span class="text-muted">(custom env)</span></span
                >
              {:else}
                <span class="text-muted">=</span>
                <span class="min-w-0 flex-1 break-all text-muted">{c.key} already set</span>
              {/if}
            </li>
          {/each}
        </ul>
        {#if noops > 0 && touches > 0}
          <p class="text-[11px] text-muted">
            {noops} already set. Recipes only add — applying never turns anything off.
          </p>
        {/if}
      {/if}
    </div>
  </Popover>
</span>
