<script lang="ts">
  import { app } from "$lib/state.svelte";
</script>

<div class="grid grid-cols-1 gap-3 sm:grid-cols-[1fr_1fr_auto]">
  {@render field("Game .exe", "umuExe", "/path/to/game.exe")}
  {@render field("WINEPREFIX", "umuWineprefix", "(optional) /path/to/prefix")}
  {@render field("GAMEID", "umuGameid", "umu-0", "9rem")}
</div>
<p class="mt-2 text-xs text-muted">
  PROTONPATH is taken from the selected Proton runtime above.
</p>

{#snippet field(label: string, key: "umuExe" | "umuWineprefix" | "umuGameid", placeholder: string, width = "auto")}
  <label class="block" style={width !== "auto" ? `width:${width}` : ""}>
    <span class="mb-1 block text-[11px] uppercase tracking-wider text-muted">{label}</span>
    <input
      value={app[key]}
      oninput={(e) => (app[key] = e.currentTarget.value)}
      {placeholder}
      class="w-full rounded-lg border border-border bg-surface-2 px-2.5 py-2 font-mono text-xs text-text outline-none focus:border-accent"
    />
  </label>
{/snippet}
