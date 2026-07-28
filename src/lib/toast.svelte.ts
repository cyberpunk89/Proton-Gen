export type ToastVariant = "success" | "error" | "info";

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface Toast {
  id: number;
  message: string;
  variant: ToastVariant;
  action: ToastAction | null;
}

interface ShowOptions {
  variant?: ToastVariant;
  ms?: number;
  action?: ToastAction;
}

/** Beyond this, the stack covers too much of the window; oldest is dropped. */
const MAX_VISIBLE = 3;

const DEFAULT_MS = 2500;
/** An actionable toast has to outlive the time it takes to read and click it. */
const ACTION_MS = 6000;

class ToastState {
  items = $state<Toast[]>([]);

  #nextId = 1;
  #timers = new Map<number, ReturnType<typeof setTimeout>>();

  show(message: string, opts: ShowOptions = {}): number {
    const id = this.#nextId++;
    const action = opts.action ?? null;

    this.items.push({
      id,
      message,
      variant: opts.variant ?? "success",
      action,
    });

    // Drop from the front so the newest — the one the user is waiting on —
    // always survives.
    while (this.items.length > MAX_VISIBLE) {
      this.#clearTimer(this.items[0].id);
      this.items.shift();
    }

    const ms = opts.ms ?? (action ? ACTION_MS : DEFAULT_MS);
    this.#timers.set(
      id,
      setTimeout(() => this.dismiss(id), ms),
    );
    return id;
  }

  success(message: string, opts: Omit<ShowOptions, "variant"> = {}) {
    return this.show(message, { ...opts, variant: "success" });
  }

  error(message: string, opts: Omit<ShowOptions, "variant"> = {}) {
    return this.show(message, { ...opts, variant: "error" });
  }

  info(message: string, opts: Omit<ShowOptions, "variant"> = {}) {
    return this.show(message, { ...opts, variant: "info" });
  }

  runAction(id: number) {
    const item = this.items.find((t) => t.id === id);
    if (!item) return;
    item.action?.onClick();
    this.dismiss(id);
  }

  dismiss(id: number) {
    this.#clearTimer(id);
    const i = this.items.findIndex((t) => t.id === id);
    if (i !== -1) this.items.splice(i, 1);
  }

  dismissAll() {
    for (const id of [...this.#timers.keys()]) this.#clearTimer(id);
    this.items = [];
  }

  #clearTimer(id: number) {
    const t = this.#timers.get(id);
    if (t !== undefined) {
      clearTimeout(t);
      this.#timers.delete(id);
    }
  }
}

export const toast = new ToastState();
