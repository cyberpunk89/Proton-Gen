<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { inTauri } from "$lib/ipc";

  /**
   * Eight invisible drag zones around the window edge.
   *
   * `decorations: false` (tauri.conf.json) means there is no server-side resize
   * border: on Wayland the compositor draws nothing to grab, so the only way to
   * resize was its own modifier+drag fallback. The custom titlebar already
   * re-implements move, minimize, maximize and close; this is the missing
   * fourth piece, so the CSD is finally complete.
   *
   * `startResizeDragging` bottoms out in `gtk_window_begin_resize_drag`
   * (xdg_toplevel.resize on Wayland, _NET_WM_MOVERESIZE on X11) — the same class
   * of call `data-tauri-drag-region` in Header.svelte already uses for moves, so
   * the transport is proven in this app.
   */

  // `ResizeDirection` in @tauri-apps/api is a type-only string union, not an
  // enum, so there is no value to import — these literals are the API.
  type Dir =
    | "North"
    | "South"
    | "East"
    | "West"
    | "NorthEast"
    | "NorthWest"
    | "SouthEast"
    | "SouthWest";

  /**
   * 4px edges, 14px corners.
   *
   * The East edge overlaps the 10px scrollbar (app.css), whose thumb is inset
   * 2px each side — so 4px still leaves 4 of the thumb's 6 visible pixels
   * directly grabbable. Widening the grips means widening the scrollbar to
   * match; do not change one without the other.
   */
  const EDGE = 4;
  const CORNER = 14;

  // Corners last so they win the overlap by paint order.
  const ZONES: { dir: Dir; cursor: string; style: string }[] = [
    { dir: "North", cursor: "n-resize", style: `top:0;left:${CORNER}px;right:${CORNER}px;height:${EDGE}px` },
    { dir: "South", cursor: "s-resize", style: `bottom:0;left:${CORNER}px;right:${CORNER}px;height:${EDGE}px` },
    { dir: "West", cursor: "w-resize", style: `left:0;top:${CORNER}px;bottom:${CORNER}px;width:${EDGE}px` },
    { dir: "East", cursor: "e-resize", style: `right:0;top:${CORNER}px;bottom:${CORNER}px;width:${EDGE}px` },
    { dir: "NorthWest", cursor: "nw-resize", style: `top:0;left:0;width:${CORNER}px;height:${CORNER}px` },
    { dir: "NorthEast", cursor: "ne-resize", style: `top:0;right:0;width:${CORNER}px;height:${CORNER}px` },
    { dir: "SouthWest", cursor: "sw-resize", style: `bottom:0;left:0;width:${CORNER}px;height:${CORNER}px` },
    { dir: "SouthEast", cursor: "se-resize", style: `bottom:0;right:0;width:${CORNER}px;height:${CORNER}px` },
  ];

  // Resizing a maximized window is meaningless — the compositor either ignores
  // it or silently unmaximizes. Nothing else in the app tracked this: the header
  // fires toggleMaximize() and forgets.
  let maximized = $state(false);

  onMount(() => {
    // Browser dev path (`pnpm dev`): no Tauri window to drive, and the first
    // invoke would reject. Same guard Header.svelte uses for the controls.
    if (!inTauri) return;

    const w = getCurrentWindow();
    let disposed = false;
    let unlisten: UnlistenFn | undefined;

    const sync = () => {
      void w.isMaximized().then((v) => {
        if (!disposed) maximized = v;
      });
    };

    sync();
    // Maximize and unmaximize always change the outer size, so onResized covers
    // both — including compositor-driven ones (Super+Up, tiling), which the
    // header button alone would miss.
    void w.onResized(sync).then((un) => {
      if (disposed) un();
      else unlisten = un;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  function grab(dir: Dir, e: PointerEvent) {
    if (e.button !== 0) return;
    // Suppresses the text caret and, in WebKit, the compatibility mousedown that
    // Tauri's own drag.js listens on.
    e.preventDefault();
    e.stopPropagation();
    void getCurrentWindow()
      .startResizeDragging(dir)
      .catch((err) => console.error("startResizeDragging failed", dir, err));
  }
</script>

{#if inTauri && !maximized}
  <!--
    z-[90] is above every piece of app chrome (the highest in flow is z-30 in
    GameTile) and above floating popovers (z-50), but deliberately BELOW the
    modal stack (z-[100] dialogs and drawer, z-[110] palette). Above it, a
    pointerdown on a grip would read as interact-outside to bits-ui's
    DismissibleLayer and dismiss the dialog.

    data-tauri-drag-region="false" is belt-and-braces: Tauri's drag.js walks the
    composed path and would already bail on a plain div, but "false"
    short-circuits the walk and states the intent.
  -->
  <div class="pointer-events-none fixed inset-0 z-[90]" data-tauri-drag-region="false" aria-hidden="true">
    {#each ZONES as z (z.dir)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        role="presentation"
        data-resize-grip={z.dir}
        class="pointer-events-auto absolute"
        style="{z.style};cursor:{z.cursor}"
        onpointerdown={(e) => grab(z.dir, e)}
      ></div>
    {/each}
  </div>
{/if}
