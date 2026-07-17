import { describe, expect, it } from "vitest";

import { formatCreated, severityColor, statusGlyph } from "./format";

describe("severityColor", () => {
  it("maps every known severity to a distinct color", () => {
    const colors = ["critical", "high", "medium", "low", "info"].map(severityColor);
    expect(new Set(colors).size).toBe(colors.length);
  });

  it("degrades unknown severities to the neutral color, never throws", () => {
    expect(severityColor("catastrophic")).toBe(severityColor("also-unknown"));
    expect(severityColor("catastrophic")).not.toBe(severityColor("critical"));
  });
});

describe("statusGlyph", () => {
  it("maps every lifecycle status", () => {
    expect(statusGlyph("investigating")).toBe("●");
    expect(statusGlyph("review")).toBe("◐");
    expect(statusGlyph("final-review")).toBe("◑");
    expect(statusGlyph("finished")).toBe("✔");
  });

  it("degrades unknown statuses to a hollow dot", () => {
    expect(statusGlyph("paused")).toBe("○");
  });
});

describe("formatCreated", () => {
  it("renders RFC 3339 timestamps as a compact UTC label", () => {
    expect(formatCreated("2026-07-15T14:32:00Z")).toBe("2026-07-15 14:32 UTC");
  });

  it("passes malformed input through unchanged", () => {
    expect(formatCreated("not a date")).toBe("not a date");
    expect(formatCreated("")).toBe("");
  });
});
