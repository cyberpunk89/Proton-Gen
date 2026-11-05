<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { SteamLogo, Package } from "phosphor-svelte";

  /** `sm` for the header bar, beside the other compact controls. */
  let { size = "md" }: { size?: "sm" | "md" } = $props();

  const modes = [
    { umu: false, label: "Steam", icon: SteamLogo },
    { umu: true, label: "umu", icon: Package },
  ];
</script>

<div
  class="inline-flex border border-border bg-surface-2/60 {size === 'sm' ? 'rounded-lg p-0.5' : 'rounded-xl p-1'}"
  role="group"
  aria-label="Command type"
>
  {#each modes as m (m.label)}
    <button
      onclick={() => app.setUmu(m.umu)}
      aria-pressed={app.umu === m.umu}
      class="inline-flex items-center gap-1.5 font-medium transition {size === 'sm'
        ? 'rounded-md px-2.5 py-1 text-xs'
        : 'rounded-lg px-3 py-1.5 text-sm'}"
      style={app.umu === m.umu
        ? "background: var(--accent); color: var(--on-accent)"
        : "color: var(--muted)"}
    >
      <m.icon size={size === "sm" ? 13 : 15} weight="fill" />
      {m.label}
    </button>
  {/each}
</div>
