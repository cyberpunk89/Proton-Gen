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

/** Focuses the node on mount. */
export const autofocus: Action<HTMLElement> = (node) => {
  queueMicrotask(() => node.focus());
};
