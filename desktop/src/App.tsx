// Composition root: wires the data hook, theme, filter, finder, and
// keybindings into the layout. Presentation lives in components/; logic
// in lib/ and hooks/.

import { useCallback, useMemo, useRef, useState } from "react";
import type { JSX } from "react";

import { DiagramView } from "./components/DiagramView";
import { FinderOverlay } from "./components/FinderOverlay";
import { HelpOverlay } from "./components/HelpOverlay";
import { IncidentHeader } from "./components/IncidentHeader";
import { SectionView } from "./components/SectionView";
import { Sidebar } from "./components/Sidebar";
import { TabBar } from "./components/TabBar";
import { useActions } from "./hooks/useActions";
import { useIncidents } from "./hooks/useIncidents";
import { useKeybindings } from "./hooks/useKeybindings";
import { useTheme } from "./hooks/useTheme";
import { filterWorkspaces } from "./lib/filter";
import type { CorpusLine } from "./lib/finder";
import { DIAGRAMS_TAB } from "./lib/sections";
import "./App.css";

export default function App(): JSX.Element {
  const { theme, toggleTheme } = useTheme();
  const incidents = useIncidents();
  const [filter, setFilter] = useState("");
  const [showArchived, setShowArchived] = useState(false);
  const [helpVisible, setHelpVisible] = useState(false);
  const [finderVisible, setFinderVisible] = useState(false);
  const filterRef = useRef<HTMLInputElement | null>(null);

  const { listing } = incidents;
  const workspaces = useMemo(() => listing?.workspaces ?? [], [listing]);
  const visible = filterWorkspaces(workspaces, filter, showArchived);
  const hiddenArchived = showArchived ? 0 : workspaces.filter((w) => w.archived).length;

  const onAction = useActions({
    visible,
    selectedId: incidents.selectedId,
    onSelect: incidents.onSelect,
    setActiveFile: incidents.setActiveFile,
    filterRef,
    toggleArchived: useCallback(() => {
      setShowArchived((current) => !current);
    }, []),
    toggleTheme,
    toggleHelp: useCallback(() => {
      setHelpVisible((current) => !current);
    }, []),
    openFinder: useCallback(() => {
      setFinderVisible(true);
    }, []),
  });
  useKeybindings({
    onAction,
    helpVisible,
    onCloseHelp: useCallback(() => {
      setHelpVisible(false);
    }, []),
  });

  const handleJump = useCallback(
    (target: CorpusLine) => {
      setFinderVisible(false);
      if (workspaces.find((w) => w.id === target.id)?.archived === true) {
        setShowArchived(true);
      }
      incidents.onSelect(target.id);
      incidents.selectTab(target.file);
    },
    [workspaces, incidents],
  );

  const { selected } = incidents;
  return (
    <main className="app">
      <Sidebar
        workspaces={visible}
        selectedId={incidents.selectedId}
        onSelect={incidents.onSelect}
        filter={filter}
        onFilterChange={setFilter}
        filterRef={filterRef}
        hiddenArchived={hiddenArchived}
        onShowArchived={() => {
          setShowArchived(true);
        }}
      />
      <section className="content">
        {incidents.error !== null ? (
          <div className="error-banner">{incidents.error}</div>
        ) : null}
        <IncidentHeader
          selected={selected}
          theme={theme}
          onToggleTheme={toggleTheme}
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
      {helpVisible ? (
        <HelpOverlay
          onClose={() => {
            setHelpVisible(false);
          }}
        />
      ) : null}
      {finderVisible ? (
        <FinderOverlay
          onJump={handleJump}
          onClose={() => {
            setFinderVisible(false);
          }}
          onError={incidents.onError}
        />
      ) : null}
    </main>
  );
}
