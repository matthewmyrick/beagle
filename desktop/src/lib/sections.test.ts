import { describe, expect, it } from "vitest";

import { SECTIONS } from "./sections";

describe("SECTIONS", () => {
  it("lists the eight sections of the on-disk format, in tab order", () => {
    expect(SECTIONS.map((s) => s.file)).toEqual([
      "summary.md",
      "timeline.md",
      "root-cause.md",
      "impact.md",
      "remediation.md",
      "final-review.md",
      "notes.md",
      "log.md",
    ]);
  });

  it("has unique files and titles", () => {
    expect(new Set(SECTIONS.map((s) => s.file)).size).toBe(SECTIONS.length);
    expect(new Set(SECTIONS.map((s) => s.title)).size).toBe(SECTIONS.length);
  });
});
