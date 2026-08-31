import type { Action } from "svelte/action";

/** Calls `handler` when a pointerdown lands outside the node. */
export const clickOutside: Action<HTMLElement, () => void> = (node, handler) => {
  let cb = handler;
  const onPointerDown = (e: PointerEvent) => {
    if (!node.contains(e.target as Node)) cb();
  };
  document.addEventListener("pointerdown", onPointerDown, true);
  return {
    update(next) {
      cb = next;
    },
    destroy() {
      document.removeEventListener("pointerdown", onPointerDown, true);
    },
  };
};

/** Focuses the node on mount, unless passed `false` — e.g. only the first of
 *  several rendered from an `{#each}` should actually take focus. */
export const autofocus: Action<HTMLElement, boolean | undefined> = (node, enabled = true) => {
  if (enabled) queueMicrotask(() => node.focus());
};

/**
 * Named focus targets, so a global key handler can focus an input without
 * reaching into the DOM. Registered by the `focusTarget` action below.
 */
export const focusRegistry = new Map<string, HTMLElement>();

/** Focuses the element registered under `name`, if it is currently mounted. */
export function focusByName(name: string): boolean {
  const el = focusRegistry.get(name);
  if (!el) return false;
  el.focus();
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) el.select();
  return true;
}

/** Registers the node under `name` for the lifetime of the component. */
export const focusTarget: Action<HTMLElement, string> = (node, name) => {
  let key = name;
  focusRegistry.set(key, node);
  return {
    update(next) {
      // Only surrender the old key if we still own it.
      if (focusRegistry.get(key) === node) focusRegistry.delete(key);
      key = next;
      focusRegistry.set(key, node);
    },
    destroy() {
      // A remount can register the replacement before the old node is torn
      // down, so never clobber someone else's entry.
      if (focusRegistry.get(key) === node) focusRegistry.delete(key);
    },
  };
};

/**
 * Calls `cb` once, when the node comes within 600px of the viewport.
 * Used for lazy-loading grid art without an arbitrary tile cap.
 */
export const inView: Action<HTMLElement, () => void> = (node, handler) => {
  let cb = handler;

  // Without IntersectionObserver, degrade to loading immediately rather than
  // never — a missing image is worse than an eager one.
  if (typeof IntersectionObserver === "undefined") {
    queueMicrotask(() => cb());
    return {
      update(next) {
        cb = next;
      },
    };
  }

  const observer = new IntersectionObserver(
    (entries) => {
      if (!entries.some((e) => e.isIntersecting)) return;
      observer.disconnect();
      cb();
    },
    { rootMargin: "600px" },
  );
  observer.observe(node);

  return {
    update(next) {
      cb = next;
    },
    destroy() {
      observer.disconnect();
    },
  };
};
