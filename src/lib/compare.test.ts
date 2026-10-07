import { describe, expect, it } from "vitest";
import { compareConfigs } from "./compare";
import { emptyConfig, type Config } from "./types";

const cfg = (over: Partial<Config>): Config => ({ ...emptyConfig(), ...over });

describe("compareConfigs", () => {
  it("compares env by key, folding in custom env", () => {
    const a = cfg({ env: [["DXVK_ASYNC", "1"], ["PROTON_LOG", "1"]] });
    const b = cfg({ env: [["DXVK_ASYNC", "0"]], extra_env: 'MY_VAR="a b"' });
    expect(compareConfigs(a, b)).toEqual([
      { label: "DXVK_ASYNC", kind: "env", a: "1", b: "0", change: "changed" },
      { label: "MY_VAR", kind: "env", a: null, b: "a b", change: "only-b" },
      { label: "PROTON_LOG", kind: "env", a: "1", b: null, change: "only-a" },
    ]);
  });

  it("ignores order and treats an argless wrapper as on", () => {
    const a = cfg({ wrappers: [["mangohud", ""], ["gamemoderun", ""]] });
    const b = cfg({ wrappers: [["gamemoderun", ""], ["mangohud", ""], ["gamescope", "-f"]] });
    expect(compareConfigs(a, b)).toEqual([
      { label: "gamescope", kind: "wrapper", a: null, b: "-f", change: "only-b" },
    ]);
  });

  it("reports mode, runtime and args, and can include unchanged rows", () => {
    const a = cfg({ umu: true, runtime: "GE-Proton10-3" });
    const b = cfg({ game_args: "-dx11" });
    expect(compareConfigs(a, b).map((r) => [r.label, r.a, r.b])).toEqual([
      ["Launch mode", "umu", "Steam"],
      ["Proton", "GE-Proton10-3", "default"],
      ["Game arguments", "—", "-dx11"],
    ]);
    expect(compareConfigs(a, a, { includeSame: true }).every((r) => r.change === "same")).toBe(true);
  });
});
