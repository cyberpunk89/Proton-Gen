import { describe, expect, it } from "vitest";
import { parseInline, parseMarkdown } from "./markdown";

describe("LLM markdown reader", () => {
  it("only ever links http(s)", () => {
    const runs = parseInline("[ok](https://protondb.com) [bad](javascript:alert(1))");
    expect(runs.filter((r) => r.href).map((r) => r.href)).toEqual(["https://protondb.com"]);
  });

  it("never treats underscores as emphasis", () => {
    const runs = parseInline("set __GL_SHADER_DISK_CACHE and PROTON_USE_WINED3D");
    expect(runs.some((r) => r.italic || r.bold)).toBe(false);
  });

  it("keeps raw HTML as plain text", () => {
    const blocks = parseMarkdown("<img src=x onerror=alert(1)>");
    expect(blocks).toEqual([{ kind: "paragraph", runs: [{ text: "<img src=x onerror=alert(1)>" }] }]);
  });

  it("parses the block kinds it supports", () => {
    const md = "# Fix\n\n- one\n- two\n\n```\nPROTON_LOG=1\n```\n\n> note\n\n---";
    expect(parseMarkdown(md).map((b) => b.kind)).toEqual(["heading", "list", "code", "quote", "rule"]);
  });
});
