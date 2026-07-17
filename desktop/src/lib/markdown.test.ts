import { describe, expect, it } from "vitest";

import { parseBlocks, parseInline } from "./markdown";

describe("parseInline", () => {
  it("splits bold and code runs", () => {
    expect(parseInline("a **b** and `c`")).toEqual([
      { kind: "text", text: "a " },
      { kind: "bold", text: "b" },
      { kind: "text", text: " and " },
      { kind: "code", text: "c" },
    ]);
  });

  it("leaves unbalanced markers literal — a stray ** never eats the line", () => {
    expect(parseInline("a ** b")).toEqual([{ kind: "text", text: "a ** b" }]);
    expect(parseInline("tick ` alone")).toEqual([{ kind: "text", text: "tick ` alone" }]);
  });
});

describe("parseBlocks", () => {
  it("parses headings, rules, and quotes", () => {
    expect(parseBlocks("# T\n## S\n---\n> hint")).toEqual([
      { kind: "heading", level: 1, text: "T" },
      { kind: "heading", level: 2, text: "S" },
      { kind: "rule" },
      { kind: "quote", text: "hint" },
    ]);
  });

  it("keeps fenced code verbatim, dropping the fence markers", () => {
    const [block] = parseBlocks("```sql\nSELECT 1;\n  -- indented\n```");
    expect(block).toEqual({ kind: "code", text: "SELECT 1;\n  -- indented" });
  });

  it("parses checkboxes and plain bullets, joining continuation lines", () => {
    expect(
      parseBlocks("- [x] done thing\n- [ ] open thing\n- plain\n  continues"),
    ).toEqual([
      { kind: "bullet", indent: 0, checkbox: "done", text: "done thing" },
      { kind: "bullet", indent: 0, checkbox: "open", text: "open thing" },
      { kind: "bullet", indent: 0, checkbox: null, text: "plain continues" },
    ]);
  });

  it("joins hard-wrapped prose into one paragraph", () => {
    expect(parseBlocks("one line\nwraps here\n\nnew paragraph")).toEqual([
      { kind: "paragraph", text: "one line wraps here" },
      { kind: "paragraph", text: "new paragraph" },
    ]);
  });

  it("never treats a checkbox inside a fence as a task", () => {
    const blocks = parseBlocks("```\n- [ ] not a task\n```");
    expect(blocks).toHaveLength(1);
    expect(blocks[0]?.kind).toBe("code");
  });
});
