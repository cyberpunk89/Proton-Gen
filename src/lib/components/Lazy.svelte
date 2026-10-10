<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import type { Loader } from "$lib/lazy";

  /**
   * Renders `children(Component)` once `load()` has resolved, starting the load
   * the first time `when` is true and then staying mounted.
   *
   * "Staying mounted" is the point for dialogs: a bits-ui modal unmounted while
   * open never runs its teardown and leaves `body { pointer-events: none }`
   * behind (#63), so this only ever adds the component, never removes it. The
   * chunks are also prefetched at idle (`prefetchLazy`), so by the time a user
   * opens one the module is normally already parsed.
   */
  let {
    load,
    when = true,
    children,
  }: {
    load: Loader;
    when?: boolean;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    children: Snippet<[Component<any>]>;
  } = $props();

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let C = $state<Component<any> | null>(null);
  let started = false;

  $effect(() => {
    if (!when || started) return;
    started = true;
    load().then(
      (m) => (C = m.default),
      (e) => {
        // Let the next open try again rather than leave the trigger dead.
        started = false;
        console.error("lazy component failed to load", e);
      },
    );
  });
</script>

{#if C}
  {@render children(C)}
{/if}
