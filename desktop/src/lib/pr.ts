// PR display helpers, pure and testable — the TS side of cli/src/prs.rs.

export type PrStateLabel = "open" | "draft" | "merged" | "closed";

/** `#123` for anything with a /pull/123 path, else a truncated URL. */
export function shortLabel(url: string): string {
  const rest = url.split("/pull/")[1];
  if (rest !== undefined) {
    const match = /^\d+/.exec(rest);
    if (match !== null) {
      return `#${match[0]}`;
    }
  }
  return url.length > 40 ? `${url.slice(0, 37)}…` : url;
}

/** One-cell status glyph, matching the TUI's. Unknown states get a dot. */
export function glyphFor(state: string | undefined): string {
  switch (state) {
    case "open":
      return "○";
    case "draft":
      return "◌";
    case "merged":
      return "✓";
    case "closed":
      return "✗";
    default:
      return "·";
  }
}
