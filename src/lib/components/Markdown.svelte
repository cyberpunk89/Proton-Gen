<script lang="ts">
  import { parseMarkdown, type Run } from "$lib/markdown";
  import { openUrl } from "$lib/util";

  /**
   * Renders LLM text as light markdown. Everything goes through `{#each}` and
   * text interpolation — there is no `{@html}` here, and there must never be:
   * the source is model output, and a prompt-injected log line could otherwise
   * put live markup in the app.
   */
  let { source }: { source: string } = $props();

  let blocks = $derived(parseMarkdown(source));

  const HEADING_TAG = { 1: "h3", 2: "h4", 3: "h5" } as const;
</script>

{#snippet runs(list: Run[])}
  {#each list as r, i (i)}
    {#if r.code}
      <code class="rounded bg-surface-2 px-1 py-px font-mono text-[0.85em] text-text">{r.text}</code>
    {:else if r.href}
      {@const href = r.href}
      <a
        {href}
        onclick={(e) => {
          e.preventDefault();
          openUrl(href);
        }}
        class="text-accent underline underline-offset-2 hover:text-text">{r.text}</a
      >
    {:else if r.bold || r.italic}
      <span class="{r.bold ? 'font-semibold text-text' : ''} {r.italic ? 'italic' : ''}">{r.text}</span>
    {:else}
      {r.text}
    {/if}
  {/each}
{/snippet}

<div class="space-y-2 text-sm leading-relaxed text-subtext">
  {#each blocks as b, i (i)}
    {#if b.kind === "heading"}
      <svelte:element this={HEADING_TAG[b.level]} class="pt-1 font-semibold text-text {b.level === 1 ? 'text-[15px]' : 'text-sm'}">
        {@render runs(b.runs)}
      </svelte:element>
    {:else if b.kind === "paragraph"}
      <p class="whitespace-pre-line">{@render runs(b.runs)}</p>
    {:else if b.kind === "list"}
      <ul class="space-y-1">
        {#each b.items as it, j (j)}
          <li class="flex gap-2 {it.depth ? 'pl-5' : ''}">
            <span class="shrink-0 select-none text-muted tabular-nums" aria-hidden="true">{it.marker}</span>
            <span class="min-w-0">{@render runs(it.runs)}</span>
          </li>
        {/each}
      </ul>
    {:else if b.kind === "code"}
      <pre
        class="overflow-x-auto rounded-lg border border-border bg-surface-2/70 px-3 py-2 font-mono text-xs leading-snug text-text">{b.text}</pre>
    {:else if b.kind === "quote"}
      <blockquote class="whitespace-pre-line border-l-2 border-border pl-3 text-muted">
        {@render runs(b.runs)}
      </blockquote>
    {:else if b.kind === "rule"}
      <hr class="border-border" />
    {:else if b.kind === "table"}
      <div class="overflow-x-auto">
        <table class="w-full border-collapse text-xs">
          <thead>
            <tr>
              {#each b.head as cell, k (k)}
                <th class="border-b border-border px-2 py-1 text-left font-semibold text-text">{@render runs(cell)}</th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each b.rows as row, r (r)}
              <tr>
                {#each row as cell, k (k)}
                  <td class="border-b border-border/50 px-2 py-1 align-top">{@render runs(cell)}</td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/each}
</div>
