import { describe, expect, it } from "vitest";
import { buildOptiScaler, emptyOptiScaler, parseOptiScaler } from "./optiscaler";

describe("OptiPatcher fix", () => {
  it("emits Plugins.LoadAsiPlugins=true when ticked", () => {
    const c = emptyOptiScaler();
    c.fixes.optiPatcher = true;
    expect(buildOptiScaler(c)).toBe("FrameGen.Enabled=false;Plugins.LoadAsiPlugins=true");
  });

  it("round-trips without landing in passthrough", () => {
    const c = parseOptiScaler("Menu.ShortcutKey=0xBF;plugins.loadasiplugins=TRUE");
    expect(c.fixes.optiPatcher).toBe(true);
    expect(c.passthrough).toEqual([]);
    expect(buildOptiScaler(c)).toBe("FrameGen.Enabled=false;Menu.ShortcutKey=0xBF;Plugins.LoadAsiPlugins=true");
  });
});

describe("frame generation off", () => {
  // The prefix's OptiScaler.ini outlives the launch line, so "off" has to be said.
  it("is written explicitly", () => {
    expect(buildOptiScaler(emptyOptiScaler())).toBe("FrameGen.Enabled=false");
  });

  it("round-trips once and doesn't duplicate", () => {
    const c = parseOptiScaler("FrameGen.Enabled=false;Menu.ShortcutKey=0xBF");
    expect(c.frameGenOn).toBe(false);
    expect(c.passthrough).toEqual([]);
    expect(buildOptiScaler(c)).toBe("FrameGen.Enabled=false;Menu.ShortcutKey=0xBF");
  });

  it("turning it off drops the input/output keys", () => {
    const c = parseOptiScaler("FrameGen.Enabled=true;FrameGen.FGInput=dlssg;FrameGen.FGOutput=fsrfg");
    c.frameGenOn = false;
    expect(buildOptiScaler(c)).toBe("FrameGen.Enabled=false");
  });
});
