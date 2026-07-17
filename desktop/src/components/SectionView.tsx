// The content pane for one section. First slice: raw markdown in a
// scrollable pre — a real markdown renderer (shared with the web app) is
// the next slice; absent sections show a hint, mirroring the TUI.

import type { JSX } from "react";

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
  return <pre className="section-content">{content}</pre>;
}
