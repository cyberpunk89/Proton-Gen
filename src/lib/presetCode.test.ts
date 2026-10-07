import { describe, expect, it } from "vitest";
import { decodePreset, encodePreset, isPresetCode, PRESET_CODE_PREFIX } from "./presetCode";
import { emptyConfig } from "./types";

const known = ["gamemoderun", "mangohud", "gamescope"];

function encodeRaw(obj: unknown): string {
  const bytes = new TextEncoder().encode(JSON.stringify(obj));
  return PRESET_CODE_PREFIX + btoa(String.fromCharCode(...bytes));
}

describe("preset share codes", () => {
  it("round-trips a preset, minus the sender's launch target", () => {
    const config = emptyConfig();
    config.env = [["PROTON_ENABLE_WAYLAND", "1"]];
    config.wrappers = [["gamemoderun", ""]];
    config.umu_exe = "/home/someone/Games/x.exe";
    const code = encodePreset({ name: "Wayland — ünïcode", config });
    expect(isPresetCode(code)).toBe(true);

    const r = decodePreset(code, known);
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.preset.name).toBe("Wayland — ünïcode");
    expect(r.preset.config.env).toEqual([["PROTON_ENABLE_WAYLAND", "1"]]);
    expect(r.preset.config.umu_exe).toBe("");
  });

  it("drops and reports wrappers this catalog doesn't know", () => {
    const r = decodePreset(
      encodeRaw({ name: "x", config: { wrappers: [["gamemoderun", ""], ["evil-wrapper", ""]] } }),
      known,
    );
    expect(r.ok && r.droppedWrappers).toEqual(["evil-wrapper"]);
    expect(r.ok && r.preset.config.wrappers).toEqual([["gamemoderun", ""]]);
  });

  it.each([
    ["not a code", "isn't a protongen preset"],
    [PRESET_CODE_PREFIX + "%%%", "damaged"],
    [encodeRaw({ name: "x" }), "no preset"],
    [encodeRaw({ name: "", config: {} }), "name is invalid"],
    [encodeRaw({ name: "x", config: { env: [["BAD KEY", "1"]] } }), "invalid variable"],
    [encodeRaw({ name: "x", config: { game_args: "a\u0000b" } }), "invalid value"],
  ])("rejects %j", (code, msg) => {
    const r = decodePreset(code, known);
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toContain(msg);
  });
});
