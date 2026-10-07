import { describe, expect, it } from "vitest";
import { buildGamescope, emptyGamescope, gamescopeWarnings, parseGamescope } from "./gamescope";

describe("gamescope args round trip", () => {
  it.each([
    "-f",
    "-W 2560 -H 1440 -w 1920 -h 1080 -r 120 -F fsr --sharpness 5 -f",
    "-W 3840 -H 2160 -f --hdr-enabled --adaptive-sync --mangoapp -e",
    "-S integer -F pixel --framerate-limit 60 -b",
  ])("build(parse(%j)) is stable", (args) => {
    const once = buildGamescope(parseGamescope(args));
    expect(buildGamescope(parseGamescope(once))).toBe(once);
  });

  it("reads long and --flag=value spellings, emits the short form", () => {
    const c = parseGamescope("--output-width=2560 --output-height 1440 --nested-refresh=144 --fullscreen --steam");
    expect(c).toMatchObject({ outW: "2560", outH: "1440", refresh: "144", fullscreen: true, steam: true });
    expect(buildGamescope(c)).toBe("-W 2560 -H 1440 -r 144 -f -e");
  });

  it("maps the old -U / -Y upscaler flags to -F", () => {
    expect(parseGamescope("-U").filter).toBe("fsr");
    expect(parseGamescope("-Y").filter).toBe("nis");
  });

  it("keeps flags it doesn't model, in order, re-quoted", () => {
    const c = parseGamescope('-f --expose-wayland --cursor "/my cursors/a.png" --backend sdl');
    expect(c.passthrough).toEqual(["--expose-wayland", "--cursor", "/my cursors/a.png", "--backend", "sdl"]);
    expect(buildGamescope(c)).toBe('-f --expose-wayland --cursor "/my cursors/a.png" --backend sdl');
  });

  it("drops sharpness when no sharpening filter uses it", () => {
    const c = emptyGamescope();
    c.sharpness = "5";
    expect(buildGamescope(c)).toBe("");
    c.filter = "nis";
    expect(buildGamescope(c)).toBe("-F nis --sharpness 5");
  });

  it("a value flag with no value is kept as passthrough, not swallowed", () => {
    const c = parseGamescope("-W -f");
    expect(c.outW).toBe("");
    expect(c.fullscreen).toBe(true);
    expect(c.passthrough).toEqual(["-W"]);
  });
});

describe("gamescope warnings", () => {
  it("flags half-set sizes, useless filters and double overlays", () => {
    const c = parseGamescope("-W 1920 -H 1080 -w 1920 -h 1080 -F fsr --mangoapp");
    const w = gamescopeWarnings(c, { mangohudWrapper: true });
    expect(w.some((x) => x.includes("only does something"))).toBe(true);
    expect(w.some((x) => x.includes("--mangoapp"))).toBe(true);
    expect(gamescopeWarnings(parseGamescope("-w 1280"), { mangohudWrapper: false })).toContain(
      "Set both game width and height, or neither.",
    );
    expect(gamescopeWarnings(parseGamescope("-W 2560 -H 1440 -f"), { mangohudWrapper: false })).toEqual([]);
  });
});
