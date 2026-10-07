import { describe, expect, it } from "vitest";
import { buildOverrides, parseOverrides } from "./dlloverrides";

describe("WINEDLLOVERRIDES", () => {
  it("splits entries at ; and each at its first =", () => {
    expect(parseOverrides("dxgi=n,b;mscoree,mshtml=d;winmm=n")).toEqual([
      { dlls: "dxgi", mode: "n,b" },
      { dlls: "mscoree,mshtml", mode: "d" },
      { dlls: "winmm", mode: "n" },
    ]);
  });

  it("reads an empty mode as disabled and drops .dll suffixes", () => {
    expect(parseOverrides("mscoree.dll=")).toEqual([{ dlls: "mscoree", mode: "d" }]);
  });

  it("keeps entries it can't parse verbatim", () => {
    const rows = parseOverrides("dxgi=n,b;*d3d9=n;weird");
    expect(rows[1]).toEqual({ dlls: "", mode: "", raw: "*d3d9=n" });
    expect(buildOverrides(rows)).toBe("dxgi=n,b;*d3d9=n;weird");
  });

  it("round-trips and skips empty rows", () => {
    const v = "dxgi=n,b;version=b,n;d3d11=n";
    expect(buildOverrides(parseOverrides(v))).toBe(v);
    expect(buildOverrides([{ dlls: " ", mode: "n" }, { dlls: "dxgi.dll", mode: "n,b" }])).toBe("dxgi=n,b");
  });
});
