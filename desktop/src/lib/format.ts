// Pure presentation helpers: colors, glyphs, and labels for severities
// and statuses. Unknown values (a newer backend than frontend) degrade to
// a neutral style instead of crashing — malformed input never takes a
// beagle frontend down.

import type { Severity, Status } from "../types";

const SEVERITY_COLORS: Readonly<Record<Severity, string>> = {
  critical: "#ff5555",
  high: "#ff9955",
  medium: "#f1fa8c",
  low: "#8be9fd",
  info: "#6272a4",
};

const STATUS_GLYPHS: Readonly<Record<Status, string>> = {
  investigating: "●",
  review: "◐",
  agent: "⚙",
  "final-review": "◑",
  finished: "✔",
};

const NEUTRAL = "#9aa0b0";

/** Badge color for a severity; neutral gray for values we don't know. */
export function severityColor(severity: string): string {
  return isSeverity(severity) ? SEVERITY_COLORS[severity] : NEUTRAL;
}

/** Status glyph; a hollow dot for values we don't know. */
export function statusGlyph(status: string): string {
  return isStatus(status) ? STATUS_GLYPHS[status] : "○";
}

/** `2026-07-15T14:32:00Z` → `2026-07-15 14:32 UTC`; bad input unchanged. */
export function formatCreated(iso: string): string {
  const match = /^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2})/.exec(iso);
  if (match === null) {
    return iso;
  }
  const [, day, time] = match;
  if (day === undefined || time === undefined) {
    return iso;
  }
  return `${day} ${time} UTC`;
}

function isSeverity(value: string): value is Severity {
  return value in SEVERITY_COLORS;
}

function isStatus(value: string): value is Status {
  return value in STATUS_GLYPHS;
}
