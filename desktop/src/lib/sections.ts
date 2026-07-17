// The eight markdown sections every workspace can contain, in tab order.
// Mirrors `SectionKind` in the CLI crate — file names are the on-disk
// format, which is the public API shared by every beagle frontend.

export interface Section {
  /** File name inside the workspace directory. */
  readonly file: string;
  /** Human-readable tab title. */
  readonly title: string;
}

export const SECTIONS: readonly Section[] = [
  { file: "summary.md", title: "Summary" },
  { file: "timeline.md", title: "Timeline" },
  { file: "root-cause.md", title: "Root Cause" },
  { file: "impact.md", title: "Impact" },
  { file: "remediation.md", title: "Fix" },
  { file: "final-review.md", title: "Final Review" },
  { file: "notes.md", title: "Notes" },
  { file: "log.md", title: "Log" },
] as const;

/** The Diagrams pseudo-tab: not a markdown section, rendered unwrapped. */
export const DIAGRAMS_TAB: Section = { file: "__diagrams__", title: "Diagrams" };

/**
 * Every tab in TUI order: the six narrative sections, Diagrams, then
 * Notes and Log.
 */
export const TABS: readonly Section[] = [
  ...SECTIONS.slice(0, 6),
  DIAGRAMS_TAB,
  ...SECTIONS.slice(6),
];

/** The tab after/before `file` in TABS, wrapping at both ends. */
export function cycleTab(file: string, direction: 1 | -1): string {
  const index = TABS.findIndex((tab) => tab.file === file);
  const from = index === -1 ? 0 : index;
  const next = (from + direction + TABS.length) % TABS.length;
  return TABS[next]?.file ?? file;
}
