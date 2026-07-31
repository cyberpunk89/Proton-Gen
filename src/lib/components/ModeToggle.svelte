<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { SteamLogo, Package } from "phosphor-svelte";

  const modes = [
    { umu: false, label: "Steam", icon: SteamLogo },
    { umu: true, label: "umu", icon: Package },
  ];
</script>

<div class="inline-flex rounded-xl border border-border bg-surface-2/60 p-1">
  {#each modes as m (m.label)}
    <button
      onclick={() => app.setUmu(m.umu)}
      class="inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium transition"
      style={app.umu === m.umu
        ? "background: var(--accent); color: var(--on-accent)"
        : "color: var(--muted)"}
    >
      <m.icon size={15} weight="fill" />
      {m.label}
    </button>
  {/each}
</div>
