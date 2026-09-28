// Shell-word helpers for the custom-env field and the browser mock's builder.
// Pure and dependency-free on purpose: mock.ts imports this, and util.ts pulls
// in ./ipc (which imports mock.ts), so living in util.ts would form a cycle.

/**
 * Quote-aware split of a `K=V K=V …` string, mirroring `parser::tokenize`
 * (src-tauri/src/parser.rs) so `FOO="a b"` stays one token.
 *
 * Kept in lockstep with the Rust side deliberately: the "custom env" chips in
 * ActiveOptions must agree with what `compose::parse_extra_env` will actually
 * put on the command line, or removing a chip would edit a token the backend
 * never saw.
 */
export function tokenizeEnv(input: string): string[] {
  // POSIX word splitting, like `parser::words`: `'…'` is literal; inside `"…"`
  // a backslash escapes only $ ` " \ and newline; outside quotes it makes the
  // next character literal.
  const tokens: string[] = [];
  const chars = [...input];
  let cur = "";
  let quote: string | null = null;
  let has = false;
  for (let i = 0; i < chars.length; i++) {
    const ch = chars[i];
    if (quote === "'") {
      if (ch === "'") quote = null;
      else cur += ch;
      continue;
    }
    if (quote === '"') {
      if (ch === '"') quote = null;
      else if (ch === "\\" && i + 1 < chars.length && '$`"\\\n'.includes(chars[i + 1])) {
        i++;
        if (chars[i] !== "\n") cur += chars[i];
      } else cur += ch;
      continue;
    }
    if (/\s/.test(ch)) {
      if (has) {
        tokens.push(cur);
        cur = "";
        has = false;
      }
      continue;
    }
    has = true;
    if (ch === '"' || ch === "'") quote = ch;
    else if (ch === "\\") {
      i++;
      if (i < chars.length && chars[i] !== "\n") cur += chars[i];
    } else cur += ch;
  }
  if (has) tokens.push(cur);
  return tokens;
}

/**
 * Quote one shell word only when it needs it — the twin of `builder::sh_quote`
 * (src-tauri/src/builder.rs): double quotes so `$VAR` still expands, escaping
 * only `"`, `\` and backtick, with a leading `~/` left outside the quotes.
 */
export function shQuote(s: string): string {
  if (s === "") return '""';
  if (/^[A-Za-z0-9_@%+=:,./~$-]+$/.test(s)) return s;
  const tilde = s.startsWith("~/") ? "~/" : "";
  const rest = tilde ? s.slice(2) : s;
  return `${tilde}"${rest.replace(/["\\`]/g, (c) => `\\${c}`)}"`;
}

/** `K=V` pairs from the custom-env field, alongside the raw token so a caller
 *  can remove exactly what it displayed. */
export function splitExtraEnv(input: string): { raw: string; key: string; value: string }[] {
  return tokenizeEnv(input).flatMap((raw) => {
    const at = raw.indexOf("=");
    if (at < 0) return [];
    return [{ raw, key: raw.slice(0, at), value: raw.slice(at + 1) }];
  });
}

/**
 * Render pairs back into the custom-env field's `K=V K=V …` form, quoted the
 * way the builder quotes them. Mirrors `compose::format_extra_env`
 * (src-tauri/src/compose.rs) and is the exact inverse of `tokenizeEnv`.
 */
export function formatExtraEnv(pairs: [string, string][]): string {
  return pairs.map(([k, v]) => (v === "" ? `${k}=` : `${k}=${shQuote(v)}`)).join(" ");
}

/**
 * Append `pairs` to an extra-env string, skipping any key it already assigns.
 * Mirrors `compose::merge_into_extra_env` (src-tauri/src/compose.rs), including
 * the tie-break: dedup is **by key and the incoming pair loses**, because these
 * pairs carry values from a catalog the app no longer has, and what the user
 * typed into the visible field outranks that.
 */
export function mergeIntoExtraEnv(extraEnv: string, pairs: [string, string][]): string {
  const existing = new Set(splitExtraEnv(extraEnv).map((p) => p.key));
  const fresh = pairs.filter(([k]) => !existing.has(k));
  if (fresh.length === 0) return extraEnv;
  const rendered = formatExtraEnv(fresh);
  return extraEnv.trim() === "" ? rendered : `${extraEnv.trimEnd()} ${rendered}`;
}

/**
 * Set `key` in an extra-env string, replacing any existing assignment of it —
 * the opposite tie-break to `mergeIntoExtraEnv`: this is the user applying a
 * new value, so it wins. Everything else is re-rendered with the builder's
 * quoting, so a value with spaces or `;` never lands unquoted.
 */
export function setInExtraEnv(extraEnv: string, key: string, value: string): string {
  const out = tokenizeEnv(extraEnv)
    .filter((t) => !t.includes("=") || t.slice(0, t.indexOf("=")) !== key)
    .map((t) => {
      const at = t.indexOf("=");
      return at < 0 ? shQuote(t) : formatExtraEnv([[t.slice(0, at), t.slice(at + 1)]]);
    });
  out.push(formatExtraEnv([[key, value]]));
  return out.join(" ");
}
