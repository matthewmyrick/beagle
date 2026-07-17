import { describe, expect, it } from "vitest";

import { plainLead, renderMarkdown } from "./render";

// A stub renderer that echoes its input, so these tests stay pure.
const echo = { parse: (src: string): string => `<rendered>${src}</rendered>` };

describe("renderMarkdown", () => {
  it("strips the leading heading before rendering", () => {
    expect(renderMarkdown(echo, "# Impact\n\nbody")).toBe("<rendered>body</rendered>");
  });

  it("strips a scaffold hint before rendering", () => {
    expect(renderMarkdown(echo, "> _placeholder_\n\nreal")).toBe(
      "<rendered>real</rendered>",
    );
  });

  it("rejects an async renderer — the build cannot await", () => {
    const asyncRenderer = { parse: (): Promise<string> => Promise.resolve("x") };
    expect(() => renderMarkdown(asyncRenderer, "x")).toThrow(/synchronous/);
  });
});

describe("plainLead", () => {
  it("takes the first paragraph and drops inline markdown", () => {
    const md = "# Summary\n\nThe **redis** pool hit `8` limits.\n\nSecond para.";
    expect(plainLead(md)).toBe("The redis pool hit 8 limits.");
  });

  it("is empty for empty input", () => {
    expect(plainLead("")).toBe("");
  });
});
