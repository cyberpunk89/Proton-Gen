import { describe, expect, it, vi } from "vitest";

// A stand-in IntersectionObserver that records instances and lets the test
// fire intersections by hand.
class FakeIO {
  static instances: FakeIO[] = [];
  observed = new Set<unknown>();
  constructor(public cb: (entries: { target: unknown; isIntersecting: boolean }[]) => void) {
    FakeIO.instances.push(this);
  }
  observe(n: unknown) {
    this.observed.add(n);
  }
  unobserve(n: unknown) {
    this.observed.delete(n);
  }
  disconnect() {
    this.observed.clear();
  }
  fire(target: unknown, isIntersecting = true) {
    this.cb([{ target, isIntersecting }]);
  }
}

describe("inView", () => {
  it("shares one observer, fires each node once, and stops watching it", async () => {
    vi.stubGlobal("IntersectionObserver", FakeIO);
    const { inView } = await import("./actions");
    const a = {} as HTMLElement;
    const b = {} as HTMLElement;
    const hitsA = vi.fn();
    const hitsB = vi.fn();
    inView(a, hitsA);
    const bAction = inView(b, hitsB);

    expect(FakeIO.instances).toHaveLength(1);
    const io = FakeIO.instances[0];
    io.fire(a, false);
    expect(hitsA).not.toHaveBeenCalled();
    io.fire(a);
    io.fire(a);
    expect(hitsA).toHaveBeenCalledTimes(1);
    expect(io.observed.has(a)).toBe(false);

    // The latest handler from `update` is the one that runs.
    const replaced = vi.fn();
    (bAction as { update: (cb: () => void) => void }).update(replaced);
    io.fire(b);
    expect(hitsB).not.toHaveBeenCalled();
    expect(replaced).toHaveBeenCalledTimes(1);

    // Destroyed before intersecting: never fires.
    const c = {} as HTMLElement;
    const hitsC = vi.fn();
    (inView(c, hitsC) as { destroy: () => void }).destroy();
    io.fire(c);
    expect(hitsC).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });
});
