// Global keybindings, TUI-style. Keys never fire while the user is
// typing in an input, and modifier chords are left to the OS/webview.

import { useEffect } from "react";

import { actionForKey } from "../lib/keys";
import type { Action } from "../lib/keys";

export interface KeyHandlers {
  onAction: (action: Action, tabIndex?: number) => void;
  /** While the help overlay is open, any key just closes it. */
  helpVisible: boolean;
  onCloseHelp: () => void;
}

function isTyping(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

export function useKeybindings({
  onAction,
  helpVisible,
  onCloseHelp,
}: KeyHandlers): void {
  useEffect(() => {
    const listener = (event: KeyboardEvent): void => {
      if (event.metaKey || event.ctrlKey || event.altKey) {
        return;
      }
      if (isTyping(event.target)) {
        return;
      }
      if (helpVisible) {
        event.preventDefault();
        onCloseHelp();
        return;
      }
      const match = actionForKey(event.key);
      if (match !== null) {
        event.preventDefault();
        onAction(match.action, match.tabIndex);
      }
    };
    window.addEventListener("keydown", listener);
    return () => {
      window.removeEventListener("keydown", listener);
    };
  }, [onAction, helpVisible, onCloseHelp]);
}
