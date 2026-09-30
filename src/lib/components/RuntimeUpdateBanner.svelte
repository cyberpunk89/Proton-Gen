<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { openUrl } from "$lib/util";
  import type { RuntimeUpdate } from "$lib/types";
  import { ArrowCircleUp, ArrowSquareOut, X } from "phosphor-svelte";

  // Read-only: protongen reports a newer build, the user installs it. Proton
  // installation is a non-goal (design.md §1.4), so there is no install button.
  function name(u: RuntimeUpdate): string {
    return u.family === "ge-proton" ? "GE-Proton" : "proton-cachyos";
  }

  function versions(u: RuntimeUpdate): { latest: string; installed: string } {
    // GE's versions already read "GE-Proton11-7"; cachyos's are bare build dates.
    return u.family === "ge-proton"
      ? { latest: u.latest, installed: u.installed }
      : { latest: `proton-cachyos build ${u.latest}`, installed: `build ${u.installed}` };
  }

  function how(u: RuntimeUpdate): string {
    if (u.installed_kind === "system")
      return "It comes with a system update (sudo pacman -Syu) once the CachyOS repos pick it up.";
    const umu =
      u.family === "ge-proton"
        ? " In umu mode, the “GE-Proton (latest)” runtime already downloads it for you."
        : "";
    return `Extract it into compatibilitytools.d (or use ProtonUp-Qt), then restart Steam.${umu}`;
  }
</script>

{#each app.visibleRuntimeUpdates as u (u.tag)}
  {@const v = versions(u)}
  <div
    class="flex items-center gap-3 rounded-xl border px-4 py-2.5"
    style="border-color: color-mix(in srgb, var(--accent) 35%, transparent);
           background: color-mix(in srgb, var(--accent) 8%, transparent)"
  >
    <ArrowCircleUp size={16} class="shrink-0 text-accent" />
    <p class="flex-1 text-xs text-subtext">
      <span class="font-medium text-text">{v.latest}</span> is out — you have
      <span class="font-medium text-text">{v.installed}</span>. {how(u)}
    </p>
    <button
      onclick={() => openUrl(u.html_url)}
      class="inline-flex shrink-0 items-center gap-1 rounded-lg bg-surface-2 px-2 py-1 text-xs text-subtext transition hover:text-text"
    >
      Release notes <ArrowSquareOut size={11} />
    </button>
    <button
      onclick={() => app.dismissRuntimeUpdate(u.tag)}
      class="shrink-0 text-muted hover:text-text"
      aria-label="Dismiss {name(u)} {v.latest}"
    >
      <X size={14} />
    </button>
  </div>
{/each}
