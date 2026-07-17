// Workspace data state: the listing, the selection, the active tab, and
// the section content that follows them. App composes this with the
// presentation-side hooks (theme, keybindings, filter).

import { useCallback, useEffect, useState } from "react";

import { listWorkspaces, readSection } from "../api";
import { DIAGRAMS_TAB, SECTIONS } from "../lib/sections";
import type { Listing, Workspace } from "../types";

const FIRST_SECTION = SECTIONS[0]?.file ?? "summary.md";

/** The last section fetch that completed, keyed by what it was for. */
interface LoadedSection {
  id: string;
  file: string;
  body: string | null;
}

export interface Incidents {
  listing: Listing | null;
  error: string | null;
  onError: (message: string) => void;
  selectedId: string | null;
  selected: Workspace | null;
  onSelect: (id: string) => void;
  activeFile: string;
  setActiveFile: (update: (current: string) => string) => void;
  selectTab: (file: string) => void;
  content: string | null;
  loading: boolean;
}

export function useIncidents(): Incidents {
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
    if (selectedId === null || activeFile === DIAGRAMS_TAB.file) {
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

  const onSelect = useCallback((id: string) => {
    setSelectedId(id);
    setActiveFile(FIRST_SECTION);
  }, []);

  const onError = useCallback((message: string) => {
    setError(message);
  }, []);

  const selectTab = useCallback((file: string) => {
    setActiveFile(file);
  }, []);

  // Loading is derived, not stored: the pane is loading whenever the last
  // completed fetch isn't for what's on screen.
  const current =
    section !== null && section.id === selectedId && section.file === activeFile
      ? section
      : null;

  return {
    listing,
    error,
    onError,
    selectedId,
    selected: listing?.workspaces.find((w) => w.id === selectedId) ?? null,
    onSelect,
    activeFile,
    setActiveFile,
    selectTab,
    content: current?.body ?? null,
    loading: selectedId !== null && current === null,
  };
}
