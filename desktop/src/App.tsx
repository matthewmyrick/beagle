// Composition root: wires the data hook, theme, filter, finder, and
// keybindings into the layout. Presentation lives in components/; logic
// in lib/ and hooks/.

import { useCallback, useMemo, useRef, useState } from "react";
import type { JSX } from "react";

import { AgentsView } from "./components/AgentsView";
import { FinderOverlay } from "./components/FinderOverlay";
import { HelpOverlay } from "./components/HelpOverlay";
import { RcaContent } from "./components/RcaContent";
import { Sidebar } from "./components/Sidebar";
import { useActions } from "./hooks/useActions";
import { useIncidents } from "./hooks/useIncidents";
import { useKeybindings } from "./hooks/useKeybindings";
import { usePrStates } from "./hooks/usePrStates";
import { useTheme } from "./hooks/useTheme";
import { filterWorkspaces } from "./lib/filter";
import type { CorpusLine } from "./lib/finder";
import "./App.css";

export default function App(): JSX.Element {
  const { theme, toggleTheme } = useTheme();
  const incidents = useIncidents();
  const [filter, setFilter] = useState("");
  const [showArchived, setShowArchived] = useState(false);
  const [helpVisible, setHelpVisible] = useState(false);
  const [finderVisible, setFinderVisible] = useState(false);
  const [showAgents, setShowAgents] = useState(false);
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
  const prStates = usePrStates(selected?.prs ?? []);
  const openAgents = (): void => {
    setShowAgents(true);
  };
  const closeAgents = (): void => {
    setShowAgents(false);
  };

  if (showAgents) {
    return <AgentsView onBack={closeAgents} />;
  }

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
        onOpenAgents={openAgents}
      />
      <RcaContent
        incidents={incidents}
        selected={selected}
        prStates={prStates}
        theme={theme}
        onToggleTheme={toggleTheme}
      />
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
