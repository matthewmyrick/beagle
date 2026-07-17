// Small pure text helpers for turning RCA files into public content.
// Pure and dependency-free so they unit-test without a DOM or filesystem.

// eslint-disable-next-line no-control-regex -- matching ANSI escapes is the point
const ANSI = /\[[0-9;]*m/g;

/** Removes ANSI SGR color codes from diagram text (the web renders plain). */
export function stripAnsi(text: string): string {
  return text.replace(ANSI, "");
}

/**
 * Drops a leading `# Heading` line from a section's markdown — the public
 * page supplies its own section heading, so the file's title would double
 * up. Leaves the rest untouched; a section with no leading H1 is returned
 * unchanged.
 */
export function stripLeadingHeading(md: string): string {
  const withoutBom = md.replace(/^﻿/, "");
  if (/^#\s+/.test(withoutBom)) {
    const newline = withoutBom.indexOf("\n");
    return newline === -1 ? "" : withoutBom.slice(newline + 1).replace(/^\n+/, "");
  }
  return withoutBom;
}

/**
 * Drops a leading scaffold hint — the `> _…_` blockquote `beagle new`
 * writes into empty sections — so a half-filled section never leaks
 * placeholder text onto the public page.
 */
export function stripScaffoldHint(md: string): string {
  return md.replace(/^\s*>\s*_.*_\s*(\n|$)/, "").replace(/^\n+/, "");
}
