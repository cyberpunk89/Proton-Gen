import { describe, expect, it } from "vitest";
import { mockApplyRecipe, mockBootstrap, mockParseCommand, mockPreviewRecipe } from "./mock";
import { emptyConfig } from "./types";

// The browser-dev mocks stand in for parse_command / apply_recipe under
// `pnpm dev`; they only need to be faithful enough that import and recipe
// apply visibly work there.
describe("browser mock: parse_command", () => {
  it("splits a Steam line into rows, custom env, wrappers and game args", () => {
    const p = mockParseCommand('DXVK_ASYNC=1 MY_VAR="a b" strangle mangohud %command% -dx11 -novid');
    expect(p.config.env).toEqual([["DXVK_ASYNC", "1"]]);
    expect(p.config.extra_env).toBe('MY_VAR="a b"');
    expect(p.config.wrappers).toEqual([["mangohud", ""]]);
    expect(p.config.game_args).toBe("-dx11 -novid");
    expect(p.dropped).toEqual(["strangle"]);
  });

  it("reads a umu line's lead vars and exe", () => {
    const p = mockParseCommand("WINEPREFIX=/p GAMEID=umu-0 PROTONPATH=/x umu-run /g/game.exe -w");
    expect(p.config).toMatchObject({ umu: true, umu_wineprefix: "/p", umu_gameid: "umu-0", umu_exe: "/g/game.exe", game_args: "-w" });
  });
});

describe("browser mock: apply_recipe", () => {
  it("applies exactly what the preview promised", () => {
    const index = mockBootstrap.recipes.findIndex((r) => mockPreviewRecipe(mockBootstrap.recipes.indexOf(r), emptyConfig()).length > 0);
    expect(index).toBeGreaterThanOrEqual(0);
    const out = mockApplyRecipe(index, emptyConfig());
    expect(mockPreviewRecipe(index, out).every((c) => c.kind === "no_op")).toBe(true);
  });
});
