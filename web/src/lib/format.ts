// Presentation helpers: severity/status labels, colors, and dates. Pure,
// and tolerant of values a newer beagle might emit — unknowns degrade to
// neutral rather than throwing.

export type Severity = "critical" | "high" | "medium" | "low" | "info";
export type Status = "investigating" | "review" | "final-review" | "finished";

const SEVERITY_COLORS: Readonly<Record<Severity, string>> = {
  critical: "#e5484d",
  high: "#e8801f",
  medium: "#d9a800",
  low: "#3a9ea5",
  info: "#7a8194",
};

const SEVERITY_LABELS: Readonly<Record<Severity, string>> = {
  critical: "Critical",
  high: "High",
  medium: "Medium",
  low: "Low",
  info: "Info",
};

const NEUTRAL = "#7a8194";

/** Accent color for a severity; neutral gray for anything unrecognized. */
export function severityColor(severity: string): string {
  return isKey(SEVERITY_COLORS, severity) ? SEVERITY_COLORS[severity] : NEUTRAL;
}

/** Capitalized severity label; the raw value if unrecognized. */
export function severityLabel(severity: string): string {
  return isKey(SEVERITY_LABELS, severity) ? SEVERITY_LABELS[severity] : severity;
}

/** A client-facing state line: finished incidents read as "Resolved". */
export function statusLabel(status: string): string {
  return status === "finished" ? "Resolved" : "Ongoing";
}

/** Whether the incident is resolved (drives the resolved styling). */
export function isResolved(status: string): boolean {
  return status === "finished";
}

/** `2026-07-15T14:32:00Z` → `July 15, 2026`; the input back if unparseable. */
export function formatDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return iso;
  }
  return date.toLocaleDateString("en-US", {
    year: "numeric",
    month: "long",
    day: "numeric",
    timeZone: "UTC",
  });
}

function isKey<T extends object>(obj: T, key: string): key is Extract<keyof T, string> {
  return Object.prototype.hasOwnProperty.call(obj, key);
}
