import { describe, expect, it } from "vitest";

import { cycleTab, DIAGRAMS_TAB, SECTIONS, TABS } from "./sections";

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

describe("TABS", () => {
  it("inserts Diagrams seventh, matching the TUI's tab order", () => {
    expect(TABS).toHaveLength(9);
    expect(TABS[6]).toEqual(DIAGRAMS_TAB);
    expect(TABS.map((t) => t.title)).toEqual([
      "Summary",
      "Timeline",
      "Root Cause",
      "Impact",
      "Fix",
      "Final Review",
      "Diagrams",
      "Notes",
      "Log",
    ]);
  });

  it("keeps the diagrams pseudo-file distinct from every real section", () => {
    expect(SECTIONS.some((s) => s.file === DIAGRAMS_TAB.file)).toBe(false);
  });
});

describe("cycleTab", () => {
  it("wraps in both directions", () => {
    expect(cycleTab("summary.md", 1)).toBe("timeline.md");
    expect(cycleTab("summary.md", -1)).toBe("log.md");
    expect(cycleTab("log.md", 1)).toBe("summary.md");
  });

  it("recovers from an unknown file by starting at the first tab", () => {
    expect(cycleTab("nonsense.md", 1)).toBe("timeline.md");
  });
});
