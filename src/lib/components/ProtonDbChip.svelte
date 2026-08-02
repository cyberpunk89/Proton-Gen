<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { ipc } from "$lib/ipc";
  import { mergeStyle, openUrl, tierColor, tierForeground, tierRank } from "$lib/util";
  import Popover from "./Popover.svelte";
  import {
    Trophy,
    ArrowSquareOut,
    ArrowClockwise,
    TrendUp,
    TrendDown,
    Medal,
  } from "phosphor-svelte";

  /**
   * Purely presentational now: the fetch, the de-duping and the result all live
   * in the store's session cache (`app.requestTier`). Previously this component
   * held the tier in local state and refetched on every game change, so bouncing
   * between two games hit protondb.com four times instead of twice.
   */

  let appId = $derived(app.selectedAppId);
  let tier = $derived(appId == null ? undefined : app.tierFor(appId));
  let loading = $derived(appId != null && app.tierLoading[String(appId)] === true);

  /**
   * `trendingTier` and `bestReportedTier` were parsed by the backend and then
   * thrown away by the UI. They carry the signal the overall tier can't:
   * "gold overall, trending bronze" is a recent regression, and that is exactly
   * what someone about to tune launch options needs to know.
   *
   * Only rendered when they disagree with the overall tier — three identical
   * pills would be noise. ProtonDB reports "unknown" for games with too few
   * recent reports, so those are dropped too.
   */
  function delta(other: string | undefined, overall: string) {
    if (!other || other === overall) return null;
    const a = tierRank(other);
    const b = tierRank(overall);
    if (a == null || b == null) return null;
    return { tier: other, better: a < b };
  }

  let trending = $derived(tier ? delta(tier.trending, tier.tier) : null);
  let best = $derived(tier ? delta(tier.best, tier.tier) : null);

  async function open() {
    if (appId == null) return;
    openUrl(await ipc.protondbUrl(appId));
  }
</script>

{#if appId == null}
  <!-- nothing to look up -->
{:else if loading}
  <span class="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs text-muted">
    <ArrowClockwise size={13} class="animate-spin" /> Checking…
  </span>
{:else if tier}
  <div class="inline-flex flex-wrap items-center gap-x-2 gap-y-1">
    <!-- The pill is the popover trigger: it replaces a `title` tooltip, which
         was keyboard-inaccessible and could only ever be one line of text. -->
    <Popover width="17rem" align="start" trapFocus={false}>
      {#snippet trigger({ props, open: isOpen })}
        <button
          {...props}
          type="button"
          class="rounded-full px-2.5 py-1 text-xs font-medium capitalize transition {isOpen
            ? 'ring-2 ring-accent'
            : ''}"
          style={mergeStyle(
            props,
            `background: ${tierColor(tier.tier)}`,
            `color: ${tierForeground(tier.tier)}`,
            "box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--text) 20%, transparent)",
          )}
          aria-label="ProtonDB rating: {tier.tier}. Show details"
        >
          {tier.tier}
        </button>
      {/snippet}

      <div class="space-y-2 text-sm">
        <p class="text-xs text-muted">
          Community reports on protondb.com, not a guarantee for your hardware.
        </p>
        <dl class="space-y-1 text-xs">
          {@render stat("Overall", tier.tier, true)}
          {@render stat("Trending", tier.trending, true)}
          {@render stat("Best reported", tier.best, true)}
          {@render stat("Confidence", tier.confidence, false)}
          {@render stat("Reports", String(tier.total), false)}
        </dl>
        <button
          type="button"
          onclick={open}
          class="inline-flex items-center gap-1 text-xs text-blue hover:underline"
        >
          <ArrowSquareOut size={13} weight="bold" /> View on ProtonDB
        </button>
      </div>
    </Popover>

    {#if trending}
      {@const Icon = trending.better ? TrendUp : TrendDown}
      <span
        class="inline-flex items-center gap-1 text-xs text-muted"
        title="Recent reports rate this {trending.better ? 'better' : 'worse'} than its overall tier"
      >
        <Icon size={13} color={tierColor(trending.tier)} />
        trending <span class="capitalize" style="color: {tierColor(trending.tier)}"
          >{trending.tier}</span
        >
      </span>
    {/if}

    {#if best?.better}
      <!-- Only when it beats the overall tier: "best: bronze" under a gold
           rating is arithmetically impossible, so a worse `best` is bad data
           rather than something to show. -->
      <span
        class="inline-flex items-center gap-1 text-xs text-muted"
        title="The best result anyone has reported"
      >
        <Medal size={13} color={tierColor(best.tier)} />
        best <span class="capitalize" style="color: {tierColor(best.tier)}">{best.tier}</span>
      </span>
    {/if}

    <span class="text-xs text-muted">{tier.total} reports</span>

    <button onclick={open} class="text-muted hover:text-blue" aria-label="Open ProtonDB page">
      <ArrowSquareOut size={14} />
    </button>
  </div>
{:else if tier === null}
  <!-- Cached failure: offer a retry rather than silently hiding. -->
  <button
    onclick={() => app.retryTier(appId)}
    class="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-xs text-red"
    title="Couldn't reach ProtonDB"
  >
    <ArrowClockwise size={13} /> Retry
  </button>
{:else}
  <button
    onclick={() => app.requestTier(appId)}
    class="inline-flex items-center gap-1.5 rounded-lg border border-border bg-surface-2/60 px-2.5 py-1.5 text-xs text-subtext transition hover:border-accent/50"
  >
    <Trophy size={13} /> ProtonDB
  </button>
{/if}

{#snippet stat(label: string, value: string, isTier: boolean)}
  <div class="flex items-baseline justify-between gap-3">
    <dt class="text-muted">{label}</dt>
    <dd class="capitalize {isTier && value !== 'unknown' ? 'font-medium' : 'text-subtext'}">
      {#if isTier && value !== "unknown"}
        <span class="inline-flex items-center gap-1.5">
          <span
            class="size-2 rounded-full"
            style="background: {tierColor(value)}; box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--text) 25%, transparent)"
          ></span>
          <span style="color: var(--text)">{value}</span>
        </span>
      {:else}
        {value}
      {/if}
    </dd>
  </div>
{/snippet}
