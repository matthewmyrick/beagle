// Markdown → HTML for public sections, and a plain-text lead for cards.
// The `marked` instance is passed in so the file stays pure and testable
// with a stub.

import { stripLeadingHeading, stripScaffoldHint } from "./text";

/** A minimal markdown renderer surface — `marked.parse` (sync). */
export interface MarkdownRenderer {
  parse(src: string): string | Promise<string>;
}

/**
 * Renders a section's markdown to HTML, first dropping the file's leading
 * `# Heading` (the page supplies its own) and any scaffold hint blockquote.
 * Synchronous — the build has no place to await, and marked is sync here.
 */
export function renderMarkdown(renderer: MarkdownRenderer, md: string): string {
  const body = stripScaffoldHint(stripLeadingHeading(md));
  const html = renderer.parse(body);
  if (typeof html !== "string") {
    throw new Error("markdown renderer must be synchronous in the build");
  }
  return html;
}

/**
 * The first paragraph of a summary as clean plain text — for index cards.
 * Strips the leading heading/hint and the common inline markdown marks.
 */
export function plainLead(md: string): string {
  const body = stripScaffoldHint(stripLeadingHeading(md));
  const firstParagraph = body.split(/\n\s*\n/)[0] ?? "";
  return firstParagraph
    .replace(/\*\*(.+?)\*\*/g, "$1")
    .replace(/`(.+?)`/g, "$1")
    .replace(/\[(.+?)\]\(.+?\)/g, "$1")
    .replace(/\s+/g, " ")
    .trim();
}
