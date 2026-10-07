import { describe, expect, it } from "vitest";
import { fuzzy, subsequence } from "./fuzzy";

describe("fuzzy", () => {
  it("subsequence finds in-order hits or null", () => {
    expect(subsequence("proton_log", "plg")).toEqual([0, 7, 9]);
    expect(subsequence("dxvk", "kd")).toBeNull();
  });

  it("AND-s whitespace-separated tokens in any order", () => {
    expect(fuzzy("DXVK_HDR enable HDR", "hdr dxvk")).not.toBeNull();
    expect(fuzzy("DXVK_HDR", "dxvk nvapi")).toBeNull();
  });

  it("falls back to keywords for a token missing from the text", () => {
    expect(fuzzy("PROTON_ENABLE_WAYLAND", "display", ["Display / Wayland"])).not.toBeNull();
  });

  it("returns merged highlight ranges", () => {
    expect(fuzzy("mangohud", "mango")?.ranges).toEqual([[0, 5]]);
  });

  it("ranks a prefix above a scattered match", () => {
    const prefix = fuzzy("gamescope", "game")!.score;
    const scattered = fuzzy("dlss-swapper and more", "game")?.score ?? 0;
    expect(prefix).toBeGreaterThan(scattered);
  });
});
