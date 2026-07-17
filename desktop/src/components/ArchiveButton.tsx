// Archive / unarchive the selected incident from the header. Archiving
// requires `finished` (the backend enforces it and its error explains
// the sign-off path); unarchiving is always allowed.

import type { JSX } from "react";

import { archiveWorkspace, unarchiveWorkspace } from "../api";
import type { Workspace } from "../types";

interface ArchiveButtonProps {
  workspace: Workspace;
  onDone: () => void;
  onError: (message: string) => void;
}

export function ArchiveButton({
  workspace,
  onDone,
  onError,
}: ArchiveButtonProps): JSX.Element | null {
  if (!workspace.archived && workspace.status !== "finished") {
    return null; // only finished incidents archive; the CLI is the escape hatch
  }
  const action = workspace.archived ? unarchiveWorkspace : archiveWorkspace;
  return (
    <button
      type="button"
      className="archive-button"
      onClick={() => {
        action(workspace.id)
          .then(onDone)
          .catch((cause: unknown) => {
            onError(String(cause));
          });
      }}
    >
      {workspace.archived ? "⤴ unarchive" : "⤵ archive"}
    </button>
  );
}
