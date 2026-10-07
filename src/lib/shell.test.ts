import { describe, expect, it } from "vitest";
import fixture from "../../src-tauri/testdata/shell.json";
import {
  formatExtraEnv,
  mergeIntoExtraEnv,
  setInExtraEnv,
  shQuote,
  splitExtraEnv,
  tokenizeEnv,
} from "./shell";

// The same table drives builder.rs / parser.rs tests, so a case added there
// fails here until the TS twin agrees (and vice versa).
describe("shared fixture with the Rust twins", () => {
  it.each(fixture.quote as [string, string][])("shQuote(%j) = %j", (input, want) => {
    expect(shQuote(input)).toBe(want);
  });

  it.each(fixture.tokenize as [string, string[]][])("tokenizeEnv(%j)", (input, want) => {
    expect(tokenizeEnv(input)).toEqual(want);
  });
});

describe("extra-env helpers", () => {
  it("formatExtraEnv is the inverse of tokenizeEnv", () => {
    const pairs: [string, string][] = [
      ["DXVK_CONFIG", "dxgi.maxFrameLatency=1;dxvk.hud=fps"],
      ["SPACED", "a b"],
      ["EMPTY", ""],
      ["QUOTED", 'say "hi"'],
    ];
    const back = splitExtraEnv(formatExtraEnv(pairs)).map((p) => [p.key, p.value]);
    expect(back).toEqual(pairs);
  });

  it("mergeIntoExtraEnv keeps what the user typed when keys collide", () => {
    expect(mergeIntoExtraEnv("FOO=mine", [["FOO", "theirs"], ["BAR", "1"]])).toBe("FOO=mine BAR=1");
    expect(mergeIntoExtraEnv("FOO=1", [["FOO", "2"]])).toBe("FOO=1");
    expect(mergeIntoExtraEnv("", [["A", "x y"]])).toBe('A="x y"');
  });

  it("setInExtraEnv replaces an existing assignment and re-quotes the rest", () => {
    expect(setInExtraEnv('FOO=1 BAR="a b"', "FOO", "2")).toBe('BAR="a b" FOO=2');
    expect(setInExtraEnv("", "X", "a;b")).toBe('X="a;b"');
  });
});
