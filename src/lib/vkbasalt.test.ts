import { describe, expect, it } from "vitest";
import { buildVkBasalt, emptyVkBasalt, parseVkBasalt } from "./vkbasalt";

describe("vkBasalt.conf round trip", () => {
  it("is stable through parse(build())", () => {
    const c = emptyVkBasalt();
    c.casOn = true;
    c.casSharpness = "0.6";
    c.smaaOn = true;
    c.toggleKey = "Prior";
    c.enableOnLaunch = false;
    const once = buildVkBasalt(c);
    expect(buildVkBasalt(parseVkBasalt(once))).toBe(once);
  });

  it("never drops settings or effects it doesn't model (comments are documented as dropped)", () => {
    const text = [
      "effects = cas:deband:myshader",
      "casSharpness = 0.4",
      "debandRange = 16",
      "myshader = /home/x/shaders/my.fx",
    ].join("\n");
    const out = buildVkBasalt(parseVkBasalt(text));
    for (const kept of ["deband", "myshader", "debandRange = 16", "myshader = /home/x/shaders/my.fx"]) {
      expect(out).toContain(kept);
    }
  });
});
