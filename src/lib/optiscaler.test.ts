import { describe, expect, it } from "vitest";
import { buildOptiScaler, emptyOptiScaler, parseOptiScaler } from "./optiscaler";

describe("OptiPatcher fix", () => {
  it("emits Plugins.LoadAsiPlugins=true when ticked", () => {
    const c = emptyOptiScaler();
    c.fixes.optiPatcher = true;
    expect(buildOptiScaler(c)).toBe("Plugins.LoadAsiPlugins=true");
  });

  it("round-trips without landing in passthrough", () => {
    const c = parseOptiScaler("Menu.ShortcutKey=0xBF;plugins.loadasiplugins=TRUE");
    expect(c.fixes.optiPatcher).toBe(true);
    expect(c.passthrough).toEqual([]);
    expect(buildOptiScaler(c)).toBe("Menu.ShortcutKey=0xBF;Plugins.LoadAsiPlugins=true");
  });
});
