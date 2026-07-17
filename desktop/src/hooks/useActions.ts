// Dispatches keybinding actions onto app state. Kept out of App.tsx so
// the composition root stays readable and under the size lint.

import { useCallback } from "react";
import type { RefObject } from "react";

import { cycleTab, TABS } from "../lib/sections";
import type { Action } from "../lib/keys";
import type { Workspace } from "../types";

export interface ActionContext {
  visible: readonly Workspace[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  setActiveFile: (update: (current: string) => string) => void;
  filterRef: RefObject<HTMLInputElement | null>;
  toggleArchived: () => void;
  toggleTheme: () => void;
  toggleHelp: () => void;
  openFinder: () => void;
}

export function useActions(
  context: ActionContext,
): (action: Action, tabIndex?: number) => void {
  const {
    visible,
    selectedId,
    onSelect,
    setActiveFile,
    filterRef,
    toggleArchived,
    toggleTheme,
    toggleHelp,
    openFinder,
  } = context;

  return useCallback(
    (action: Action, tabIndex?: number) => {
      switch (action) {
        case "next-incident":
        case "previous-incident": {
          const step = action === "next-incident" ? 1 : -1;
          const index = visible.findIndex((w) => w.id === selectedId);
          const next = visible[Math.min(visible.length - 1, Math.max(0, index + step))];
          if (next !== undefined) {
            onSelect(next.id);
          }
          break;
        }
        case "next-tab":
          setActiveFile((current) => cycleTab(current, 1));
          break;
        case "previous-tab":
          setActiveFile((current) => cycleTab(current, -1));
          break;
        case "jump-tab": {
          const tab = tabIndex === undefined ? undefined : TABS[tabIndex];
          if (tab !== undefined) {
            setActiveFile(() => tab.file);
          }
          break;
        }
        case "focus-filter":
          filterRef.current?.focus();
          break;
        case "toggle-archived":
          toggleArchived();
          break;
        case "toggle-theme":
          toggleTheme();
          break;
        case "toggle-help":
          toggleHelp();
          break;
        case "open-finder":
          openFinder();
          break;
      }
    },
    [
      visible,
      selectedId,
      onSelect,
      setActiveFile,
      filterRef,
      toggleArchived,
      toggleTheme,
      toggleHelp,
      openFinder,
    ],
  );
}
