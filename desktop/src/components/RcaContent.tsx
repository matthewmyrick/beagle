// The RCA browser's content pane: error banner, incident header, tab bar, and
// the active section or diagram. Lifted out of the composition root so App
// stays a lean wiring layer.

import type { JSX } from "react";

import type { Incidents } from "../hooks/useIncidents";
import { DIAGRAMS_TAB } from "../lib/sections";
import type { Theme } from "../lib/theme";
import type { Workspace } from "../types";
import { DiagramView } from "./DiagramView";
import { IncidentHeader } from "./IncidentHeader";
import { SectionView } from "./SectionView";
import { TabBar } from "./TabBar";

interface RcaContentProps {
  incidents: Incidents;
  selected: Workspace | null;
  prStates: Record<string, string>;
  theme: Theme;
  onToggleTheme: () => void;
}

export function RcaContent({
  incidents,
  selected,
  prStates,
  theme,
  onToggleTheme,
}: RcaContentProps): JSX.Element {
  return (
    <section className="content">
      {incidents.error !== null ? (
        <div className="error-banner">{incidents.error}</div>
      ) : null}
      <IncidentHeader
        selected={selected}
        prStates={prStates}
        theme={theme}
        onToggleTheme={onToggleTheme}
        onArchiveDone={incidents.reload}
        onError={incidents.onError}
      />
      {selected !== null ? (
        <>
          <TabBar activeFile={incidents.activeFile} onSelect={incidents.selectTab} />
          {incidents.activeFile === DIAGRAMS_TAB.file ? (
            <DiagramView id={selected.id} onError={incidents.onError} />
          ) : (
            <SectionView
              content={incidents.content}
              loading={incidents.loading}
              file={incidents.activeFile}
            />
          )}
        </>
      ) : (
        <div className="section-hint">
          No RCA workspaces under {incidents.listing?.root ?? "the current directory"} —
          create one with `beagle new`.
        </div>
      )}
    </section>
  );
}
