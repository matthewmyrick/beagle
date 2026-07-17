// Composition root: sidebar + header + tabs + section content. State
// lives here; components below it are presentational.

import { useCallback, useEffect, useState } from "react";
import type { JSX } from "react";

import { listWorkspaces, readSection } from "./api";
import { Sidebar } from "./components/Sidebar";
import { SectionView } from "./components/SectionView";
import { TabBar } from "./components/TabBar";
import { SECTIONS } from "./lib/sections";
import type { Listing, Workspace } from "./types";
import "./App.css";

const FIRST_SECTION = SECTIONS[0]?.file ?? "summary.md";

/** The last section fetch that completed, keyed by what it was for. */
interface LoadedSection {
  id: string;
  file: string;
  body: string | null;
}

export default function App(): JSX.Element {
  const [listing, setListing] = useState<Listing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [activeFile, setActiveFile] = useState<string>(FIRST_SECTION);
  const [section, setSection] = useState<LoadedSection | null>(null);

  useEffect(() => {
    listWorkspaces()
      .then((result) => {
        setListing(result);
        setSelectedId((current) => current ?? result.workspaces[0]?.id ?? null);
      })
      .catch((cause: unknown) => {
        setError(String(cause));
      });
  }, []);

  useEffect(() => {
    if (selectedId === null) {
      return undefined;
    }
    let stale = false;
    readSection(selectedId, activeFile)
      .then((body) => {
        if (!stale) {
          setSection({ id: selectedId, file: activeFile, body });
        }
      })
      .catch((cause: unknown) => {
        if (!stale) {
          setError(String(cause));
        }
      });
    return () => {
      stale = true;
    };
  }, [selectedId, activeFile]);

  const handleSelect = useCallback((id: string) => {
    setSelectedId(id);
    setActiveFile(FIRST_SECTION);
  }, []);

  // Loading is derived, not stored: the pane is loading whenever the last
  // completed fetch isn't for what's on screen.
  const current =
    section !== null && section.id === selectedId && section.file === activeFile
      ? section
      : null;
  const loading = selectedId !== null && current === null;

  const selected: Workspace | null =
    listing?.workspaces.find((w) => w.id === selectedId) ?? null;

  return (
    <main className="app">
      <Sidebar
        workspaces={listing?.workspaces ?? []}
        selectedId={selectedId}
        onSelect={handleSelect}
      />
      <section className="content">
        {error !== null ? <div className="error-banner">{error}</div> : null}
        {selected !== null ? (
          <>
            <header className="incident-header">
              <h1>{selected.title}</h1>
              <p className="incident-meta">
                {selected.status} · {selected.severity} · {selected.systems.join(", ")}
              </p>
            </header>
            <TabBar activeFile={activeFile} onSelect={setActiveFile} />
            <SectionView
              content={current?.body ?? null}
              loading={loading}
              file={activeFile}
            />
          </>
        ) : (
          <div className="section-hint">
            No RCA workspaces under {listing?.root ?? "the current directory"} — create
            one with `beagle new`.
          </div>
        )}
      </section>
    </main>
  );
}
