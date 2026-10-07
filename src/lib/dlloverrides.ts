/**
 * WINEDLLOVERRIDES <-> a list of (dlls, mode) rows, for the DLL-overrides
 * editor. Wine's syntax: entries separated by `;`, each `dll[,dll…]=mode`,
 * where mode is `n`, `b`, `n,b`, `b,n`, or empty / `d` (disabled). A mode can
 * itself hold a comma (`n,b`), so an entry splits at its first `=` only.
 * Anything that doesn't fit is kept verbatim as a raw row, never dropped.
 */

export const MODES = [
  { value: "n,b", label: "Native, then builtin" },
  { value: "n", label: "Native only" },
  { value: "b,n", label: "Builtin, then native" },
  { value: "b", label: "Builtin only" },
  { value: "d", label: "Disabled" },
];

/** Overrides people reach for, with why — one click adds the row. */
export const COMMON: { dlls: string; mode: string; why: string }[] = [
  { dlls: "dxgi", mode: "n,b", why: "OptiScaler / ReShade / DLSS mods loaded as dxgi.dll" },
  { dlls: "winmm", mode: "n,b", why: "Mods and ASI loaders using winmm.dll" },
  { dlls: "version", mode: "n,b", why: "Mods and ASI loaders using version.dll" },
  { dlls: "dinput8", mode: "n,b", why: "Script hooks / mod loaders as dinput8.dll" },
  { dlls: "d3d11", mode: "n", why: "A game-bundled d3d11.dll (e.g. a ReShade build)" },
  { dlls: "mscoree,mshtml", mode: "d", why: "Skip the Mono/Gecko install prompt" },
];

export interface Override {
  /** Comma-separated DLL names, without `.dll`. */
  dlls: string;
  mode: string;
  /** Set when the entry couldn't be parsed; it is emitted as-is. */
  raw?: string;
}

const MODE_RE = /^(n|b|n,b|b,n|d|)$/;
const DLL_RE = /^[A-Za-z0-9_.+-]+(,[A-Za-z0-9_.+-]+)*$/;

export function parseOverrides(value: string): Override[] {
  return value
    .split(";")
    .map((e) => e.trim())
    .filter(Boolean)
    .map((e) => {
      const eq = e.indexOf("=");
      if (eq < 0) return { dlls: "", mode: "", raw: e };
      const dlls = e.slice(0, eq).trim().replace(/\.dll\b/gi, "");
      const mode = e.slice(eq + 1).trim().replace(/\s+/g, "");
      if (!DLL_RE.test(dlls) || !MODE_RE.test(mode)) return { dlls: "", mode: "", raw: e };
      return { dlls, mode: mode === "" ? "d" : mode };
    });
}

export function buildOverrides(rows: Override[]): string {
  return rows
    .map((r) => {
      if (r.raw !== undefined) return r.raw.trim();
      const dlls = r.dlls
        .split(",")
        .map((d) => d.trim().replace(/\.dll$/i, ""))
        .filter(Boolean)
        .join(",");
      return dlls ? `${dlls}=${r.mode || "d"}` : "";
    })
    .filter(Boolean)
    .join(";");
}
