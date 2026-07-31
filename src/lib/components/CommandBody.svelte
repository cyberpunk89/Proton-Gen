<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { openUrl } from "$lib/util";
  import { ArrowSquareOut } from "phosphor-svelte";
  import type { Token, TokenKind } from "$lib/types";

  /**
   * The payoff surface: the command, coloured by token kind, with an inspector
   * strip for whatever is hovered or focused.
   *
   * ## Selection fidelity is the whole point of this file's shape
   *
   * The body used to be one `<button class="select-text">`. Replacing it with a
   * tokenized DOM must not break drag-select, Ctrl+A, or middle-click
   * primary-selection paste — on WebKitGTK, not Chromium. The rules below are
   * load-bearing, not style preferences:
   *
   * 1. The container is a plain `<div>`, never a `<button>`: interactive tokens
   *    cannot nest inside a button (invalid HTML, and drag-select across nested
   *    button boundaries is unreliable).
   * 2. Spacing comes *only* from `space` tokens rendered as bare text nodes. No
   *    gap, margin, flex, grid or inline-block anywhere on a token span — any
   *    block-ish display inserts implicit newlines into the copied text. Only
   *    `color`/`font-weight`/`text-decoration` are safe to set.
   * 3. No `::before`/`::after` `content:` inside the body — pseudo-element text
   *    leaks into selections in some engines.
   * 4. No `user-select: none` on any token.
   * 5. **Zero whitespace between tags** in the each-body. Svelte turns a newline
   *    between two `<span>`s into a stray space, which would corrupt the copied
   *    command. That is why the each block below is one unbroken line — do not
   *    reformat it, and do not add a formatter that would.
   * 6. `env` tokens render as three sub-spans whose text concatenates back to
   *    `t.text` exactly (split at the first `=`, both halves kept verbatim).
   * 7. Copy copies `app.command`, never the DOM (see `copy` in CommandPreview).
   *    Belt and braces on top of that, `toks` below refuses to render tokens
   *    that don't reassemble into `app.command`.
   */

  let { copy }: { copy: () => void } = $props();

  let container = $state<HTMLDivElement | null>(null);
  let hovered = $state<number | null>(null);
  let focused = $state<number | null>(null);
  /** Which entry of `keyed` holds the single tab stop (roving tabindex). */
  let rovingRaw = $state(0);

  /**
   * The tokens to render — but only if they reassemble into exactly the command
   * we would copy. Tokens lag `command` by one IPC round trip, and a mismatch
   * would mean the text on screen is not the text on the clipboard. Falling back
   * to one opaque token keeps them identical by construction.
   */
  let toks = $derived.by((): Token[] => {
    const t = app.tokens;
    if (t.length && t.map((x) => x.text).join("") === app.command) return t;
    return [{ text: app.command || "%command%", kind: "unknown", key: null }];
  });

  /** Indices of tokens worth focusing: the ones with catalog info behind them. */
  let keyed = $derived(toks.map((t, i) => (t.key ? i : -1)).filter((i) => i >= 0));
  let roving = $derived(keyed.length ? Math.min(rovingRaw, keyed.length - 1) : 0);

  // Colour only — never anything that changes `display`. See rule 2.
  const KIND_CLASS: Record<TokenKind, string> = {
    space: "",
    env: "text-blue",
    wrapper: "text-mauve",
    wrapper_arg: "text-subtext",
    separator: "text-muted",
    target: "text-accent font-medium",
    exe: "text-green",
    game_arg: "text-peach",
    unknown: "text-subtext",
  };

  const KIND_LABEL: Record<TokenKind, string> = {
    space: "",
    env: "variable",
    wrapper: "wrapper",
    wrapper_arg: "wrapper arg",
    separator: "separator",
    target: "target",
    exe: "executable",
    game_arg: "game arg",
    unknown: "unrecognised",
  };

  /** Kinds actually present, so the legend describes this command and not a
   *  hypothetical one. */
  let legend = $derived(
    [...new Set(toks.filter((t) => t.kind !== "space").map((t) => t.kind))].map((k) => ({
      kind: k,
      label: KIND_LABEL[k],
      cls: KIND_CLASS[k],
    })),
  );

  interface Info {
    title: string;
    body: string | null;
    defaultValue: string;
    values: string[];
    example: string | null;
    url: string | null;
  }

  /** A short explanation for tokens the catalog knows nothing about, so the strip
   *  is useful across the whole line rather than only over env vars. */
  function kindBlurb(t: Token): string | null {
    switch (t.kind) {
      case "target":
        return app.umu
          ? "umu-run launches the game through umu-launcher."
          : "Steam substitutes the game's own launch command here.";
      case "exe":
        return "The Windows executable umu-run will start.";
      case "game_arg":
        return "Passed straight through to the game.";
      case "separator":
        return "Ends gamescope's arguments; everything after it is the game.";
      case "wrapper_arg":
        return "An argument to the wrapper program before it.";
      case "unknown":
        return "protongen didn't emit this and can't classify it.";
      default:
        return null;
    }
  }

  function infoFor(i: number | null): Info | null {
    if (i == null) return null;
    const t = toks[i];
    if (!t) return null;

    if (t.key) {
      const e = app.catalog.envs.find((x) => x.key === t.key);
      if (e) {
        return {
          title: e.key,
          body: e.details ?? e.help,
          defaultValue: e.default_value,
          values: e.values,
          example: e.example,
          url: e.url,
        };
      }
      const w = app.catalog.wrappers.find((x) => x.key === t.key);
      if (w) {
        return {
          title: w.label ?? w.key,
          body: w.details ?? w.help,
          defaultValue: w.default_value,
          values: [],
          example: w.example,
          url: w.url,
        };
      }
    }

    const blurb = kindBlurb(t);
    if (!blurb) return null;
    return {
      title: t.text.trim(),
      body: blurb,
      defaultValue: "",
      values: [],
      example: null,
      url: null,
    };
  }

  // Hover wins over focus: the mouse is the more immediate intent.
  let info = $derived(infoFor(hovered) ?? infoFor(focused));

  /** Split an `env` token at the *first* `=`; both halves stay verbatim so the
   *  three sub-spans concatenate back to `t.text` byte-for-byte (rule 6). */
  function envParts(text: string): [string, string, string] {
    const at = text.indexOf("=");
    if (at < 0) return [text, "", ""];
    return [text.slice(0, at), "=", text.slice(at + 1)];
  }

  /**
   * Per-token attributes, built here rather than inline because the each block has
   * to stay on one unbroken line (rule 5) and would otherwise be unreadable.
   *
   * Only inspectable tokens are focusable, and exactly one of them carries
   * `tabindex="0"` — a roving tabindex, so this pinned bar adds a single tab stop
   * rather than fifteen. Attributes only: every event handler lives on the
   * container, so nothing here can interfere with selection.
   */
  function tokAttrs(t: Token, i: number) {
    const inspectable = Boolean(t.key);
    return {
      "data-tok": i,
      class:
        KIND_CLASS[t.kind] +
        (inspectable ? " cursor-help decoration-dotted underline-offset-2 hover:underline" : ""),
      tabindex: inspectable ? (keyed[roving] === i ? 0 : -1) : undefined,
      "aria-describedby": inspectable ? "command-inspector" : undefined,
    };
  }

  function tokenIndexFrom(e: Event): number | null {
    const el = e.target instanceof HTMLElement ? e.target.closest("[data-tok]") : null;
    if (!(el instanceof HTMLElement)) return null;
    const i = Number(el.dataset.tok);
    return Number.isInteger(i) ? i : null;
  }

  function focusToken(i: number) {
    container?.querySelector<HTMLElement>(`[data-tok="${i}"]`)?.focus();
  }

  function move(delta: number) {
    if (!keyed.length) return;
    const next = Math.min(Math.max(roving + delta, 0), keyed.length - 1);
    rovingRaw = next;
    focusToken(keyed[next]);
  }

  function onKeydown(e: KeyboardEvent) {
    if (!keyed.length) return;
    if (e.key === "ArrowRight") {
      e.preventDefault();
      move(1);
    } else if (e.key === "ArrowLeft") {
      e.preventDefault();
      move(-1);
    } else if (e.key === "Home") {
      e.preventDefault();
      rovingRaw = 0;
      focusToken(keyed[0]);
    } else if (e.key === "End") {
      e.preventDefault();
      rovingRaw = keyed.length - 1;
      focusToken(keyed[keyed.length - 1]);
    } else if (e.key === "Escape") {
      // Give the keyboard back rather than trapping it among the tokens.
      focused = null;
      container?.focus();
    }
  }

  function onClick(e: MouseEvent) {
    // A drag-select must not collapse itself into a copy.
    if (!window.getSelection()?.isCollapsed) return;
    const i = tokenIndexFrom(e);
    // Clicking an inspectable token inspects it; clicking anywhere else copies.
    if (i != null && toks[i]?.key) return;
    copy();
  }

  function onFocusIn(e: FocusEvent) {
    const i = tokenIndexFrom(e);
    if (i == null) return;
    focused = i;
    const at = keyed.indexOf(i);
    if (at >= 0) rovingRaw = at;
  }

  function onFocusOut(e: FocusEvent) {
    if (!container?.contains(e.relatedTarget as Node | null)) focused = null;
  }
</script>

<!--
  onmouseover / onfocusin are on the container rather than on every token: the
  events bubble, so one handler pair does the work of 2N and there is less to get
  wrong. `data-tok` carries the index.

  The each block below is ONE UNBROKEN LINE, and the `>` of the opening tag butts
  straight up against `{#each}`. Any newline or comment in between becomes a text
  node and corrupts the copied command. See rule 5 in the header.
-->
<!--
  A group holding a roving tabindex is a recognised composite-widget pattern; the
  heuristic below only models single interactive elements. Keyboard parity is
  real, not waived: `onfocusin` mirrors `onmouseover` so the strip is never
  hover-only, and arrow/Home/End/Escape are handled in `onKeydown`.
-->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  bind:this={container}
  role="group"
  aria-label="Command tokens"
  tabindex="-1"
  onclick={onClick}
  onkeydown={onKeydown}
  onmouseover={(e) => (hovered = tokenIndexFrom(e))}
  onmouseleave={() => (hovered = null)}
  onfocusin={onFocusIn}
  onfocus={onFocusIn}
  onfocusout={onFocusOut}
  class="block w-full cursor-copy select-text text-left font-mono text-[13px] leading-relaxed text-text focus-visible:outline-none"
  style="word-break: break-word"
>{#each toks as t, i (i)}{#if t.kind === "space"}{t.text}{:else}<span {...tokAttrs(t, i)}>{#if t.kind === "env"}{@const p = envParts(t.text)}<span>{p[0]}</span><span class="text-muted">{p[1]}</span><span class="text-text">{p[2]}</span>{:else}{t.text}{/if}</span>{/if}{/each}</div>

<!--
  Inspector strip: one fixed-height row, not a popover per token. No positioning
  maths, nothing overlaying the command, one instance instead of N, and keyboard
  reachable by construction. Fixed height so the pinned bar never jumps.
-->
<div id="command-inspector" class="mt-2 border-t border-border/50 pt-1.5">
  <div class="h-8 overflow-hidden text-[11px] leading-tight">
    {#if info}
      <p class="truncate">
        <code class="font-mono font-medium text-text">{info.title}</code>
        {#if info.body}<span class="text-subtext"> — {info.body}</span>{/if}
      </p>
      <p class="mt-0.5 flex items-center gap-2 truncate text-muted">
        {#if info.defaultValue}
          <span class="shrink-0">default <code class="font-mono">{info.defaultValue}</code></span>
        {/if}
        {#if info.values.length}
          <span class="truncate">values <code class="font-mono">{info.values.join(" ")}</code></span>
        {/if}
        {#if info.example}
          <span class="truncate font-mono">{info.example}</span>
        {/if}
        {#if info.url}
          {@const url = info.url}
          <button
            type="button"
            onclick={() => openUrl(url)}
            class="ml-auto inline-flex shrink-0 items-center gap-1 text-blue hover:underline"
          >
            <ArrowSquareOut size={11} weight="bold" /> Docs
          </button>
        {/if}
      </p>
    {:else}
      <p class="text-muted">
        Hover a part of the command, or focus it and use
        <kbd class="font-mono">←</kbd> <kbd class="font-mono">→</kbd>, to see what it does.
      </p>
      <p class="mt-0.5 flex flex-wrap items-center gap-x-2.5 text-muted">
        {#each legend as l (l.kind)}
          <span class="{l.cls} shrink-0">{l.label}</span>
        {/each}
      </p>
    {/if}
  </div>
</div>
