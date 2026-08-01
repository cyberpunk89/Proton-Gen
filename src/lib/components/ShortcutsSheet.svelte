<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { keys, prettyCombo } from "$lib/keys.svelte";
  import Dialog from "./Dialog.svelte";

  /**
   * Rendered from `keys.bindings` — the same array the handler dispatches on —
   * so this sheet cannot describe a shortcut that doesn't exist, or miss one
   * that does.
   */
  let groups = $derived.by(() => {
    const by = new Map<string, typeof keys.bindings>();
    for (const b of keys.bindings) {
      const list = by.get(b.group);
      if (list) list.push(b);
      else by.set(b.group, [b]);
    }
    return [...by.entries()].map(([name, items]) => ({ name, items }));
  });

  const SCOPE: Record<string, string> = {
    builder: "in the builder",
    library: "in the library",
    always: "",
  };
</script>

<Dialog bind:open={app.showShortcuts} title="Keyboard shortcuts">
  <div class="space-y-4">
    {#each groups as g (g.name)}
      <div>
        <p class="mb-1.5 text-[11px] font-medium uppercase tracking-wider text-muted">
          {g.name}
        </p>
        <ul class="space-y-1">
          {#each g.items as b (b.combo + b.when)}
            <li class="flex items-baseline gap-3 text-xs">
              <kbd
                class="shrink-0 rounded-md border border-border bg-surface-2 px-1.5 py-0.5 font-mono text-[11px] text-subtext"
              >
                {prettyCombo(b.combo)}
              </kbd>
              <span class="min-w-0 flex-1 text-subtext">
                {b.description}
                {#if SCOPE[b.when]}
                  <span class="text-muted"> — {SCOPE[b.when]}</span>
                {/if}
              </span>
            </li>
          {/each}
        </ul>
      </div>
    {/each}
    <p class="border-t border-border/50 pt-2 text-[11px] text-muted">
      Single-key shortcuts are ignored while you're typing or when a dialog is open.
    </p>
  </div>
</Dialog>
