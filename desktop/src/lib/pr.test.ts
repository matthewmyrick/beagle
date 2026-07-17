import { describe, expect, it } from "vitest";

import { glyphFor, shortLabel } from "./pr";

describe("shortLabel", () => {
  it("extracts #123 from pull URLs", () => {
    expect(shortLabel("https://github.com/acme/infra/pull/4212")).toBe("#4212");
    expect(shortLabel("https://github.com/acme/infra/pull/7?tab=files")).toBe("#7");
  });

  it("truncates non-pull URLs instead of failing", () => {
    expect(shortLabel("https://example.com/x")).toBe("https://example.com/x");
    const long = `https://example.com/${"a".repeat(60)}`;
    expect(shortLabel(long).length).toBeLessThanOrEqual(40);
    expect(shortLabel(long).endsWith("…")).toBe(true);
  });
});

describe("glyphFor", () => {
  it("matches the TUI's glyphs and degrades on unknowns", () => {
    expect(glyphFor("open")).toBe("○");
    expect(glyphFor("draft")).toBe("◌");
    expect(glyphFor("merged")).toBe("✓");
    expect(glyphFor("closed")).toBe("✗");
    expect(glyphFor("weird")).toBe("·");
    expect(glyphFor(undefined)).toBe("·");
  });
});
