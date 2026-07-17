// The content column's top strip: incident title + meta + archive action
// on the left, the brand corner (banner + theme toggle) on the right.

import type { JSX } from "react";

import { ArchiveButton } from "./ArchiveButton";
import { Brand } from "./Brand";
import type { Theme } from "../lib/theme";
import type { Workspace } from "../types";

interface IncidentHeaderProps {
  selected: Workspace | null;
  theme: Theme;
  onToggleTheme: () => void;
  onArchiveDone: () => void;
  onError: (message: string) => void;
}

export function IncidentHeader({
  selected,
  theme,
  onToggleTheme,
  onArchiveDone,
  onError,
}: IncidentHeaderProps): JSX.Element {
  return (
    <div className="content-top">
      <header className="incident-header">
        {selected !== null ? (
          <>
            <h1>{selected.title}</h1>
            <p className="incident-meta">
              {selected.status} · {selected.severity} · {selected.systems.join(", ")}
              <ArchiveButton
                workspace={selected}
                onDone={onArchiveDone}
                onError={onError}
              />
            </p>
          </>
        ) : null}
      </header>
      <Brand theme={theme} onToggleTheme={onToggleTheme} />
    </div>
  );
}
