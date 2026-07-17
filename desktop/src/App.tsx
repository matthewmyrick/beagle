// Composition root: wires the data hook, theme, filter, and keybindings
// into the layout. Presentation lives in components/; logic in lib/ and
// hooks/.

import { useCallback, useRef, useState } from "react";
import type { JSX } from "react";

import { Brand } from "./components/Brand";
import { DiagramView } from "./components/DiagramView";
import { HelpOverlay } from "./components/HelpOverlay";
import { SectionView } from "./components/SectionView";
import { Sidebar } from "./components/Sidebar";
import { TabBar } from "./components/TabBar";
import { useActions } from "./hooks/useActions";
import { useIncidents } from "./hooks/useIncidents";
import { useKeybindings } from "./hooks/useKeybindings";
import { useTheme } from "./hooks/useTheme";
import { filterWorkspaces } from "./lib/filter";
import { DIAGRAMS_TAB } from "./lib/sections";
import "./App.css";

export default function App(): JSX.Element {
  const { theme, toggleTheme } = useTheme();
  const incidents = useIncidents();
  const [filter, setFilter] = useState("");
  const [showArchived, setShowArchived] = useState(false);
  const [helpVisible, setHelpVisible] = useState(false);
  const filterRef = useRef<HTMLInputElement | null>(null);

  const workspaces = incidents.listing?.workspaces ?? [];
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
  });
  useKeybindings({
    onAction,
    helpVisible,
    onCloseHelp: useCallback(() => {
      setHelpVisible(false);
    }, []),
  });

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
      />
      <section className="content">
        {incidents.error !== null ? (
          <div className="error-banner">{incidents.error}</div>
        ) : null}
        <div className="content-top">
          <header className="incident-header">
            {selected !== null ? (
              <>
                <h1>{selected.title}</h1>
                <p className="incident-meta">
                  {selected.status} · {selected.severity} · {selected.systems.join(", ")}
                </p>
              </>
            ) : null}
          </header>
          <Brand theme={theme} onToggleTheme={toggleTheme} />
        </div>
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
    </main>
  );
}
