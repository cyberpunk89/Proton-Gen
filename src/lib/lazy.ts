import type { Component } from "svelte";

/** A dynamic import of a Svelte component module. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type Loader = () => Promise<{ default: Component<any> }>;

/**
 * Components kept out of the startup chunk: dialogs and builders nobody needs
 * to see the first frame. Each loader is the one place its `import()` is
 * written, so `Lazy` and `prefetchLazy` share the module promise.
 */
export const lazy = {
  settings: () => import("./components/SettingsDrawer.svelte"),
  logViewer: () => import("./components/LogViewer.svelte"),
  troubleshooter: () => import("./components/Troubleshooter.svelte"),
  compare: () => import("./components/CompareDialog.svelte"),
  pendingQueue: () => import("./components/PendingQueue.svelte"),
  mangohud: () => import("./components/MangoHud.svelte"),
  optiscaler: () => import("./components/OptiScaler.svelte"),
  vkbasalt: () => import("./components/VkBasalt.svelte"),
  lsfg: () => import("./components/LosslessScaling.svelte"),
  gamescope: () => import("./components/GamescopeBuilder.svelte"),
  dllOverrides: () => import("./components/DllOverrides.svelte"),
} satisfies Record<string, Loader>;

/**
 * Warm every lazy chunk once the app is idle, so opening a dialog later doesn't
 * wait on a fetch + parse. Failures are ignored here; `Lazy` retries and
 * reports on actual use.
 */
export function prefetchLazy() {
  const run = () => {
    for (const load of Object.values(lazy)) void load().catch(() => {});
  };
  if (typeof requestIdleCallback === "function") requestIdleCallback(run, { timeout: 3000 });
  else setTimeout(run, 1500);
}
