import {
  fade as svelteFade,
  fly as svelteFly,
  slide as svelteSlide,
  type FadeParams,
  type FlyParams,
  type SlideParams,
  type TransitionConfig,
} from "svelte/transition";

/**
 * app.css collapses CSS transitions under prefers-reduced-motion, but that
 * rule cannot reach Svelte's JS transitions (fade/fly/slide), which drive
 * their own timing. These wrappers close that gap.
 *
 * Reactive on purpose: reading matchMedia once at construction — as
 * CommandPreview used to — means the setting is sampled at page load and
 * never updated, so toggling it mid-session had no effect until reload.
 */
class MotionState {
  #query =
    typeof window !== "undefined" && typeof window.matchMedia === "function"
      ? window.matchMedia("(prefers-reduced-motion: reduce)")
      : null;

  reduced = $state(this.#query?.matches ?? false);

  constructor() {
    // Deliberately never removed: this lives for the lifetime of the page.
    this.#query?.addEventListener("change", (e) => (this.reduced = e.matches));
  }
}

const motion = new MotionState();

/** Reactive `prefers-reduced-motion: reduce`. Safe to read in an $effect. */
export function prefersReducedMotion(): boolean {
  return motion.reduced;
}

/**
 * Zeroing duration rather than skipping the transition keeps Svelte's
 * intro/outro lifecycle intact — an outro that is never scheduled leaves its
 * element in the DOM forever.
 */
function timing<T extends { duration?: number; delay?: number }>(params?: T): T {
  const p = { ...(params ?? {}) } as T;
  if (motion.reduced) {
    p.duration = 0;
    p.delay = 0;
  }
  return p;
}

export function fade(node: Element, params?: FadeParams): TransitionConfig {
  return svelteFade(node, timing(params));
}

export function fly(node: Element, params?: FlyParams): TransitionConfig {
  return svelteFly(node, timing(params));
}

export function slide(node: Element, params?: SlideParams): TransitionConfig {
  return svelteSlide(node, timing(params));
}
