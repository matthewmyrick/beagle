import { describe, expect, it } from "vitest";

import {
  formatDate,
  isResolved,
  severityColor,
  severityLabel,
  statusLabel,
} from "./format";

describe("severity", () => {
  it("colors every severity distinctly and labels them", () => {
    const colors = ["critical", "high", "medium", "low", "info"].map(severityColor);
    expect(new Set(colors).size).toBe(5);
    expect(severityLabel("critical")).toBe("Critical");
  });

  it("degrades unknown severities to neutral, never throws", () => {
    expect(severityColor("nuclear")).toBe(severityColor("also-unknown"));
    expect(severityLabel("nuclear")).toBe("nuclear");
  });
});

describe("status", () => {
  it("reads finished as Resolved and others as Ongoing", () => {
    expect(statusLabel("finished")).toBe("Resolved");
    expect(isResolved("finished")).toBe(true);
    expect(statusLabel("review")).toBe("Ongoing");
    expect(isResolved("investigating")).toBe(false);
  });
});

describe("formatDate", () => {
  it("renders RFC 3339 as a friendly UTC date", () => {
    expect(formatDate("2026-07-15T14:32:00Z")).toBe("July 15, 2026");
  });

  it("returns bad input unchanged", () => {
    expect(formatDate("not a date")).toBe("not a date");
  });
});
