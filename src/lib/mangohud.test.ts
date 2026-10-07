import { describe, expect, it } from "vitest";
import { buildConfig, emptyConfig, parseConfig } from "./mangohud";

describe("MANGOHUD_CONFIG round trip", () => {
  it.each([
    "fps,frame_timing",
    "fps,frame_timing,position=top-right,font_size=24,horizontal,horizontal_stretch=0,gpu_list=0,1,fps_limit=144",
    "fps,background_alpha=0.6,alpha=0.9",
  ])("build(parse(%j)) is stable", (raw) => {
    const once = buildConfig(parseConfig(raw));
    expect(buildConfig(parseConfig(once))).toBe(once);
  });

  it("keeps a comma-bearing gpu_list value together", () => {
    expect(parseConfig("fps,gpu_list=1,0").gpuList).toEqual([1, 0]);
    expect(buildConfig(parseConfig("fps,gpu_list=1,0"))).toContain("gpu_list=0,1");
  });

  it("an empty config defaults to fps + frametime", () => {
    expect(buildConfig(emptyConfig())).toBe("fps,frame_timing");
  });

  it("ignores a non-positive fps_limit", () => {
    expect(parseConfig("fps,fps_limit=0").fpsLimit).toBe("");
  });
});
