import { app } from "./state.svelte";
import { focusByName } from "./actions";
import { copyCommandAction, resetCommandAction } from "./commands";

/**
 * The app's one key-handling layer.
 *
 * Before this there was essentially no keyboard support, and what existed was
 * scattered `svelte:window` handlers that fired regardless of layering — one
 * Escape closed both a dialog and the drawer behind it. Epic 2 fixed the overlay
 * half by moving to bits-ui layers; this fixes the global half: exactly one
 * `svelte:window onkeydown`, and one table that both the handler and the `?`
 * sheet read from, so the documentation cannot drift from the behaviour.
 */

export type Scope = "always" | "builder" | "library";

export interface Binding {
  /** Display form, also the match key. "Mod" renders as Ctrl (⌘ on Mac). */
  combo: string;
  when: Scope;
  group: string;
  description: string;
  /** Single-key bindings must not fire while typing or under an overlay. */
  needsModifier: boolean;
  run: (e: KeyboardEvent) => void;
}

/** True for anywhere a keystroke is text input rather than a command. */
export function isTypingTarget(e: KeyboardEvent): boolean {
  const t = e.target;
  if (!(t instanceof HTMLElement)) return false;
  return !!t.closest("input, textarea, select, [contenteditable]:not([contenteditable=false])");
}

const isMac =
  typeof navigator !== "undefined" && /mac|iphone|ipad/i.test(navigator.platform ?? "");

/** Normalised description of the pressed keystroke, in the table's vocabulary. */
function comboOf(e: KeyboardEvent): string {
  const mod = e.ctrlKey || e.metaKey;
  const parts: string[] = [];
  if (mod) parts.push("Mod");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");

  let key = e.key;
  if (key === " ") key = "Space";
  // "?" already carries Shift; listing both would never match.
  else if (key === "?") return parts.includes("Mod") ? "Mod+?" : "?";
  else if (key.length === 1) key = key.toUpperCase();

  parts.push(key);
  return parts.join("+");
}

export function prettyCombo(combo: string): string {
  return combo.replace("Mod", isMac ? "⌘" : "Ctrl");
}

class Keys {
  /**
   * How many overlays are currently open.
   *
   * Kept here rather than read from bits-ui's internal `globalThis.bitsEscapeLayers`,
   * which is not a public contract. Overlays increment/decrement it from an
   * `$effect`; global single-key bindings only fire at zero, which is what stops
   * `/` from typing into the panel behind a modal.
   *
   * This covers keys the overlay itself ignores. For keys the overlay *handles*
   * — Escape above all — the `defaultPrevented` check in `handle` is the load
   * bearing one; see the note there.
   *
   * Deliberately NOT `$state`. It is only ever read from `handle`, a DOM event
   * callback outside any reactive scope — and making it reactive is actively
   * harmful: `overlays++` *reads* the value, so an `$effect` calling
   * `pushOverlay()` would take a dependency on the counter it just wrote and
   * re-run itself, silently re-running everything else in that effect. That cost
   * an afternoon in the palette, whose effect also resets the search query.
   */
  private overlays = 0;

  get overlayOpen(): boolean {
    return this.overlays > 0;
  }

  pushOverlay() {
    this.overlays++;
  }
  popOverlay() {
    this.overlays = Math.max(0, this.overlays - 1);
  }

  get bindings(): Binding[] {
    return BINDINGS;
  }

  /** Bindings that apply right now, for the `?` sheet. */
  get activeBindings(): Binding[] {
    return BINDINGS.filter((b) => b.when === "always" || b.when === app.view);
  }

  handle = (e: KeyboardEvent) => {
    if (!app.ready) return;

    /**
     * Someone closer to the target already dealt with this keystroke.
     *
     * This is what actually makes Escape layer correctly, and the overlay
     * counter below cannot do it: bits-ui closes its topmost layer *and* removes
     * it from the DOM synchronously during the event, so by the time this
     * window-level handler runs the dialog is gone and the counter is already
     * back to zero. Measured — `dialogsInDom` was 0 here while the sheet was
     * visibly open a moment earlier. Without this check, one Escape closed the
     * shortcuts sheet and dumped the user back to the library.
     */
    if (e.defaultPrevented) return;

    const combo = comboOf(e);
    const typing = isTypingTarget(e);

    for (const b of BINDINGS) {
      if (b.combo !== combo) continue;
      if (b.when !== "always" && b.when !== app.view) continue;
      // Single-key bindings stay out of the way of text entry and overlays;
      // modifier bindings (Mod+K, Mod+,) work everywhere by design.
      if (!b.needsModifier && (typing || this.overlayOpen)) continue;
      b.run(e);
      return;
    }
  };
}

const BINDINGS: Binding[] = [
  {
    combo: "Mod+K",
    when: "always",
    group: "General",
    description: "Open the command palette",
    needsModifier: true,
    run: (e) => {
      e.preventDefault();
      app.showPalette = !app.showPalette;
    },
  },
  {
    combo: "Mod+,",
    when: "always",
    group: "General",
    description: "Open settings",
    needsModifier: true,
    run: (e) => {
      e.preventDefault();
      app.showSettings = true;
    },
  },
  {
    combo: "?",
    when: "always",
    group: "General",
    description: "Show keyboard shortcuts",
    needsModifier: false,
    run: (e) => {
      e.preventDefault();
      app.showShortcuts = true;
    },
  },
  {
    combo: "/",
    when: "builder",
    group: "Navigation",
    description: "Search parameters",
    needsModifier: false,
    run: (e) => {
      // preventDefault or the "/" lands in the field we just focused.
      e.preventDefault();
      focusByName("param-search");
    },
  },
  {
    combo: "/",
    when: "library",
    group: "Navigation",
    description: "Filter games",
    needsModifier: false,
    run: (e) => {
      e.preventDefault();
      focusByName("library-filter");
    },
  },
  {
    combo: "Escape",
    when: "builder",
    group: "Navigation",
    description: "Back to the library",
    needsModifier: false,
    run: (e) => {
      e.preventDefault();
      app.backToLibrary();
    },
  },
  {
    combo: "Mod+C",
    when: "builder",
    group: "Command",
    description: "Copy the command",
    needsModifier: true,
    run: (e) => {
      if (isTypingTarget(e)) return;
      // A real selection means the user wants *that*, not the whole command.
      // CommandBody is select-text, so drag-select then Mod+C still works
      // natively — this only fills in the empty-selection case.
      if (window.getSelection()?.toString()) return;
      e.preventDefault();
      void copyCommandAction.run();
    },
  },
  {
    combo: "Mod+Z",
    when: "builder",
    group: "Command",
    description: "Undo",
    needsModifier: true,
    run: (e) => {
      if (isTypingTarget(e)) return; // native undo owns text fields
      e.preventDefault();
      app.undo();
    },
  },
  {
    combo: "Mod+Shift+Z",
    when: "builder",
    group: "Command",
    description: "Redo",
    needsModifier: true,
    run: (e) => {
      if (isTypingTarget(e)) return;
      e.preventDefault();
      app.redo();
    },
  },
  {
    combo: "Mod+Y",
    when: "builder",
    group: "Command",
    description: "Redo (alternative)",
    needsModifier: true,
    run: (e) => {
      if (isTypingTarget(e)) return;
      e.preventDefault();
      app.redo();
    },
  },
  {
    combo: "Mod+R",
    when: "builder",
    group: "Command",
    description: "Reset the command",
    needsModifier: true,
    run: (e) => {
      e.preventDefault();
      resetCommandAction.run();
    },
  },
];

export const keys = new Keys();
