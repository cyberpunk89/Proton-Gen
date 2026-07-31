import { emptyConfig } from "./types";
import type { Config } from "./types";

/**
 * Undo/redo for the builder.
 *
 * The design is a **funnel, not a scatter**: rather than sprinkling snapshot
 * calls across ~20 mutators (where the next one added would silently miss),
 * the root `$effect` in `state.svelte.ts` hands every post-mutation state to
 * `observe()`. A structural compare against `baseline` decides whether
 * anything actually changed, and a coalescing timer folds a burst of edits —
 * typing into a value field — into one entry.
 *
 * Discrete actions (toggling an option, applying a recipe) additionally call
 * `flush()` so their entry lands immediately carrying a legible label, instead
 * of waiting out the timer and settling for the generic one.
 *
 * Deliberately **not** persisted across restarts: `Store.last_session` already
 * restores the exact screen, and writing 50 configs into `state.toml` would
 * bloat it and force a breaking `Store` DTO change for little gain.
 */

/** How long a burst of edits folds into a single entry. */
const COALESCE_MS = 700;

/** Ring-buffer depth. A `Config` is a few hundred bytes, so this is cheap. */
const DEPTH = 50;

/** Label for a coalesced burst nobody called `note()` for. */
const GENERIC_LABEL = "edit";

/** Everything undo has to restore. `activePresetName` is not part of `Config`
 *  and shouldn't be — carry it here rather than forcing a DTO change. */
export interface Snapshot {
  config: Config;
  appId: number | null;
  gameName: string | null;
  activePresetName: string | null;
}

export interface Entry extends Snapshot {
  /** Names the action that moved us *off* this snapshot, so the undo control
   *  can say what pressing it will undo. Lowercase phrase: "enable DXVK_HUD". */
  label: string;
  at: number;
}

function freshSnapshot(): Snapshot {
  return { config: emptyConfig(), appId: null, gameName: null, activePresetName: null };
}

/** Strip `label`/`at` so a popped entry can become the baseline. The compare is
 *  a JSON compare, so stray fields would make it never match. */
function toSnapshot(e: Entry | Snapshot): Snapshot {
  return {
    config: e.config,
    appId: e.appId,
    gameName: e.gameName,
    activePresetName: e.activePresetName,
  };
}

function same(a: Snapshot, b: Snapshot): boolean {
  // A Snapshot is ~10 shallow fields plus two small string-pair arrays, built
  // in a deterministic order by toConfig(), so key order is stable.
  return JSON.stringify(a) === JSON.stringify(b);
}

class History {
  /**
   * The last committed state.
   *
   * Public because `undo()`/`redo()` must move it *before* the caller applies
   * the config: applying mutates state, which fires the root effect, which
   * calls `observe()` — and a stale baseline would make that look like a fresh
   * user edit and push a bogus entry. Assigning the baseline first is race-free
   * and needs no `applying` flag (a flag would race the effect scheduler).
   */
  baseline: Snapshot = freshSnapshot();

  undoStack = $state<Entry[]>([]);
  redoStack = $state<Entry[]>([]);

  /** Latest observed state not yet committed, i.e. the burst in progress. */
  private pending: Snapshot | null = null;
  private pendingLabel: string | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;

  get canUndo(): boolean {
    return this.undoStack.length > 0;
  }
  get canRedo(): boolean {
    return this.redoStack.length > 0;
  }
  /** What pressing undo will undo, for the button's tooltip. */
  get undoLabel(): string | null {
    return this.undoStack.at(-1)?.label ?? null;
  }
  get redoLabel(): string | null {
    return this.redoStack.at(-1)?.label ?? null;
  }

  /** Seed the baseline and drop both stacks — used once the restored session is
   *  in place, so the first `observe()` doesn't read as a user edit. */
  reset(snapshot: Snapshot) {
    this.cancelPending();
    this.baseline = snapshot;
    this.undoStack = [];
    this.redoStack = [];
  }

  /** Called from the root effect on every state change. */
  observe(snapshot: Snapshot) {
    if (same(snapshot, this.baseline)) {
      // Net no change (an option toggled on and back off): nothing to record,
      // and any burst in flight is moot.
      this.cancelPending();
      return;
    }
    this.pending = snapshot;
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => this.commitPending(), COALESCE_MS);
  }

  /** Give the burst in progress a better label than the generic one. For value
   *  edits, where the keystrokes coalesce and there is no discrete moment to
   *  flush on. */
  note(label: string) {
    this.pendingLabel = label;
  }

  /** Commit immediately with a specific label, for discrete actions. Any burst
   *  still in flight lands first as its own entry, so a recipe applied
   *  mid-typing doesn't swallow the typing. */
  flush(label: string, snapshot: Snapshot) {
    this.commitPending();
    this.commit(label, snapshot);
  }

  /**
   * Rewind one entry. Returns what the caller should apply, or null if the
   * stack is empty. The baseline has already moved by the time this returns —
   * see the field's doc comment.
   */
  undo(): Entry | null {
    this.commitPending();
    const e = this.undoStack.pop();
    if (!e) return null;
    const entry = $state.snapshot(e) as Entry;
    // The state we're leaving becomes the redo target, keeping the label so
    // "Redo: enable X" mirrors the "Undo: enable X" that got us here.
    this.redoStack.push({ ...this.baseline, label: entry.label, at: Date.now() });
    this.baseline = toSnapshot(entry);
    return entry;
  }

  redo(): Entry | null {
    // Any uncommitted burst is a new edit, and a new edit invalidates redo —
    // committing it here (which clears the stack) is the correct outcome, not
    // a bug to work around.
    this.commitPending();
    const e = this.redoStack.pop();
    if (!e) return null;
    const entry = $state.snapshot(e) as Entry;
    this.undoStack.push({ ...this.baseline, label: entry.label, at: Date.now() });
    this.baseline = toSnapshot(entry);
    return entry;
  }

  private commitPending() {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    const p = this.pending;
    this.pending = null;
    if (p) this.commit(this.pendingLabel ?? GENERIC_LABEL, p);
    this.pendingLabel = null;
  }

  private commit(label: string, snapshot: Snapshot) {
    if (same(snapshot, this.baseline)) return;
    this.undoStack.push({ ...this.baseline, label, at: Date.now() });
    if (this.undoStack.length > DEPTH) this.undoStack.shift();
    this.baseline = snapshot;
    // Any new commit forks the timeline; the old redo path is unreachable.
    this.redoStack = [];
    this.pendingLabel = null;
  }

  private cancelPending() {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    this.pending = null;
    this.pendingLabel = null;
  }
}

export const history = new History();
