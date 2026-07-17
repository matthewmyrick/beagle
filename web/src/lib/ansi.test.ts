import { describe, expect, it } from "vitest";

import { ansiToHtml, stripAnsi } from "./ansi";

// ESC built at runtime so the source stays clean ASCII.
const E = String.fromCharCode(27);

describe("stripAnsi", () => {
  it("removes SGR codes, keeps text and alignment", () => {
    expect(stripAnsi(`${E}[1;31mBUG${E}[0m -- pool: 8`)).toBe("BUG -- pool: 8");
  });

  it("leaves plain text untouched", () => {
    expect(stripAnsi("no codes")).toBe("no codes");
  });
});

describe("ansiToHtml", () => {
  it("wraps bold colored runs in classed spans", () => {
    expect(ansiToHtml(`${E}[1;31mBUG${E}[0m ok`)).toBe(
      '<span class="ansi-bold ansi-red">BUG</span> ok',
    );
  });

  it("maps green and yellow (healthy / degraded)", () => {
    expect(ansiToHtml(`${E}[32mOK${E}[0m`)).toBe('<span class="ansi-green">OK</span>');
    expect(ansiToHtml(`${E}[33mwarn${E}[0m`)).toBe(
      '<span class="ansi-yellow">warn</span>',
    );
  });

  it("resets style at 0m so later text is plain", () => {
    expect(ansiToHtml(`${E}[31mred${E}[0m plain`)).toBe(
      '<span class="ansi-red">red</span> plain',
    );
  });

  it("escapes < and > in diagram text", () => {
    expect(ansiToHtml("a <b> c")).toBe("a &lt;b&gt; c");
  });

  it("degrades plain input to escaped text", () => {
    expect(ansiToHtml("just text")).toBe("just text");
  });
});
