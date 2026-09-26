// A small markdown reader for the local LLM's answers (troubleshooter, log
// coach). It parses into plain data that `Markdown.svelte` renders with
// `{#each}` — never `{@html}` — so model output can't inject markup, however
// it's phrased. Unrecognised syntax simply stays as text.
//
// Deliberately narrow: headings, paragraphs, bullet/numbered lists (one nesting
// level), fenced code, block quotes, rules, pipe tables; inline `code`,
// **bold**, *italic* and [links](https://…). `_` is never emphasis: the text is
// full of identifiers like `__GL_SHADER_DISK_CACHE` and PROTON_USE_WINED3D.

export interface Run {
  text: string;
  bold?: boolean;
  italic?: boolean;
  code?: boolean;
  /** Only ever http(s). */
  href?: string;
}

export interface ListItem {
  /** "•", or the number the model wrote ("3.") — so a list split by blank
   *  lines still counts on instead of restarting at 1. */
  marker: string;
  depth: 0 | 1;
  runs: Run[];
}

export type Block =
  | { kind: "heading"; level: 1 | 2 | 3; runs: Run[] }
  | { kind: "paragraph"; runs: Run[] }
  | { kind: "list"; items: ListItem[] }
  | { kind: "code"; text: string }
  | { kind: "quote"; runs: Run[] }
  | { kind: "rule" }
  | { kind: "table"; head: Run[][]; rows: Run[][][] };

const FENCE = /^\s*(```|~~~)/;
const HEADING = /^\s{0,3}(#{1,6})\s+(.*?)\s*#*\s*$/;
const RULE = /^\s{0,3}([-*_])(\s*\1){2,}\s*$/;
const QUOTE = /^\s{0,3}>\s?(.*)$/;
const ITEM = /^(\s*)([-*+]|\d{1,9}[.)])\s+(.*)$/;
const TABLE_ROW = /^\s*\|.*\|\s*$/;
const TABLE_SEP = /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/;
const LINK = /^\[([^\]\n]+)\]\((https?:\/\/[^\s)]+)\)/;

export function parseMarkdown(source: string): Block[] {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];

  let para: string[] = [];
  let items: { marker: string; depth: 0 | 1; text: string }[] = [];
  let quote: string[] = [];

  const flushPara = () => {
    if (para.length) blocks.push({ kind: "paragraph", runs: parseInline(para.join("\n")) });
    para = [];
  };
  const flushList = () => {
    if (items.length) {
      blocks.push({
        kind: "list",
        items: items.map((it) => ({ marker: it.marker, depth: it.depth, runs: parseInline(it.text) })),
      });
    }
    items = [];
  };
  const flushQuote = () => {
    if (quote.length) blocks.push({ kind: "quote", runs: parseInline(quote.join("\n")) });
    quote = [];
  };
  const flushAll = () => {
    flushPara();
    flushList();
    flushQuote();
  };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    const fence = FENCE.exec(line);
    if (fence) {
      flushAll();
      const body: string[] = [];
      // An unclosed fence runs to the end, as in CommonMark.
      while (++i < lines.length && !lines[i].trimStart().startsWith(fence[1])) body.push(lines[i]);
      blocks.push({ kind: "code", text: body.join("\n") });
      continue;
    }

    if (!line.trim()) {
      flushAll();
      continue;
    }

    const heading = HEADING.exec(line);
    if (heading) {
      flushAll();
      const level = Math.min(heading[1].length, 3) as 1 | 2 | 3;
      blocks.push({ kind: "heading", level, runs: parseInline(heading[2]) });
      continue;
    }

    // Before ITEM: "- - -" and "* * *" are rules, not nested bullets.
    if (RULE.test(line)) {
      flushAll();
      blocks.push({ kind: "rule" });
      continue;
    }

    const q = QUOTE.exec(line);
    if (q) {
      flushPara();
      flushList();
      quote.push(q[1]);
      continue;
    }
    flushQuote();

    if (TABLE_ROW.test(line) && i + 1 < lines.length && TABLE_SEP.test(lines[i + 1])) {
      flushAll();
      const head = cells(line);
      const rows: Run[][][] = [];
      i++; // the separator
      while (i + 1 < lines.length && TABLE_ROW.test(lines[i + 1])) rows.push(cells(lines[++i]));
      blocks.push({ kind: "table", head, rows });
      continue;
    }

    const item = ITEM.exec(line);
    if (item) {
      flushPara();
      const marker = /\d/.test(item[2]) ? `${parseInt(item[2], 10)}.` : "•";
      items.push({ marker, depth: item[1].length >= 2 ? 1 : 0, text: item[3] });
      continue;
    }

    if (items.length) {
      // A continuation line (indented or lazy) belongs to the item above it.
      items[items.length - 1].text += " " + line.trim();
      continue;
    }

    para.push(line.trim());
  }
  flushAll();
  return blocks;
}

function cells(row: string): Run[][] {
  return row
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((c) => parseInline(c.trim()));
}

type Style = Pick<Run, "bold" | "italic">;

/** Inline spans. A delimiter with no partner is kept as literal text. */
export function parseInline(s: string, style: Style = {}): Run[] {
  const out: Run[] = [];
  let buf = "";
  const flush = () => {
    if (buf) out.push({ text: buf, ...style });
    buf = "";
  };

  let i = 0;
  while (i < s.length) {
    const c = s[i];

    if (c === "`") {
      const end = s.indexOf("`", i + 1);
      if (end > i + 1) {
        flush();
        out.push({ text: s.slice(i + 1, end), code: true });
        i = end + 1;
        continue;
      }
    } else if (c === "*" && s[i + 1] === "*") {
      const end = s.indexOf("**", i + 2);
      if (end > i + 2) {
        flush();
        out.push(...parseInline(s.slice(i + 2, end), { ...style, bold: true }));
        i = end + 2;
        continue;
      }
    } else if (c === "*" && s[i + 1] && !/\s/.test(s[i + 1])) {
      const end = closingStar(s, i + 1);
      if (end > 0) {
        flush();
        out.push(...parseInline(s.slice(i + 1, end), { ...style, italic: true }));
        i = end + 1;
        continue;
      }
    } else if (c === "[") {
      const m = LINK.exec(s.slice(i));
      if (m) {
        flush();
        out.push({ text: m[1], href: m[2], ...style });
        i += m[0].length;
        continue;
      }
    }

    buf += c;
    i++;
  }
  flush();
  return out;
}

/** Index of the `*` closing an italic span opened just before `from`, or -1.
 *  Skips `**` pairs and a `*` preceded by whitespace ("2 * 3"). */
function closingStar(s: string, from: number): number {
  for (let j = from; j < s.length; j++) {
    if (s[j] === "`") {
      const end = s.indexOf("`", j + 1);
      if (end > j) j = end;
      continue;
    }
    if (s[j] !== "*") continue;
    if (s[j + 1] === "*") {
      j++;
      continue;
    }
    if (!/\s/.test(s[j - 1])) return j;
  }
  return -1;
}
