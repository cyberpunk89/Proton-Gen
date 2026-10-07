/**
 * Two configs, side by side: which settings differ and how. Semantic, not
 * textual — env vars are compared by key (custom-env assignments folded in,
 * since that's where they end up on the command line), wrappers by key, so
 * order and quoting never register as a difference.
 */

import type { Config } from "./types";
import { splitExtraEnv } from "./shell";

export type Change = "same" | "changed" | "only-a" | "only-b";

export interface CompareRow {
  /** What the row is about: an env key, a wrapper name, or a field label. */
  label: string;
  kind: "env" | "wrapper" | "field";
  a: string | null;
  b: string | null;
  change: Change;
}

/** env + custom env as one key -> value map; the custom field wins, as on the
 *  command line (it is emitted later). */
function envMap(c: Config): Map<string, string> {
  const m = new Map<string, string>();
  for (const [k, v] of c.env) m.set(k, v);
  for (const { key, value } of splitExtraEnv(c.extra_env)) m.set(key, value);
  return m;
}

function classify(a: string | null, b: string | null): Change {
  if (a === null) return "only-b";
  if (b === null) return "only-a";
  return a === b ? "same" : "changed";
}

export function compareConfigs(a: Config, b: Config, opts: { includeSame?: boolean } = {}): CompareRow[] {
  const rows: CompareRow[] = [];

  const field = (label: string, va: string, vb: string) =>
    rows.push({ label, kind: "field", a: va || "—", b: vb || "—", change: va === vb ? "same" : "changed" });
  field("Launch mode", a.umu ? "umu" : "Steam", b.umu ? "umu" : "Steam");
  field("Proton", a.runtime ?? "default", b.runtime ?? "default");
  field("Game arguments", a.game_args.trim(), b.game_args.trim());

  const wa = new Map(a.wrappers);
  const wb = new Map(b.wrappers);
  for (const k of [...new Set([...wa.keys(), ...wb.keys()])].sort()) {
    const va = wa.has(k) ? wa.get(k) || "on" : null;
    const vb = wb.has(k) ? wb.get(k) || "on" : null;
    rows.push({ label: k, kind: "wrapper", a: va, b: vb, change: classify(va, vb) });
  }

  const ea = envMap(a);
  const eb = envMap(b);
  for (const k of [...new Set([...ea.keys(), ...eb.keys()])].sort()) {
    const va = ea.get(k) ?? null;
    const vb = eb.get(k) ?? null;
    rows.push({ label: k, kind: "env", a: va, b: vb, change: classify(va, vb) });
  }

  return opts.includeSame ? rows : rows.filter((r) => r.change !== "same");
}
