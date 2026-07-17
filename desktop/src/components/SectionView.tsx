// The content pane for one section: rendered markdown (the TUI's subset),
// fading in on change. Absent sections show a hint, mirroring the TUI.

import type { JSX } from "react";

import { Markdown } from "./Markdown";

interface SectionViewProps {
  content: string | null;
  loading: boolean;
  file: string;
}

export function SectionView({ content, loading, file }: SectionViewProps): JSX.Element {
  if (loading) {
    return <div className="section-hint">loading…</div>;
  }
  if (content === null) {
    return (
      <div className="section-hint">
        No {file} yet — it will appear here live once the investigation writes it.
      </div>
    );
  }
  return (
    <div className="section-content fade-in">
      <Markdown source={content} />
    </div>
  );
}
