// The sidebar's visible-list computation, pure and testable: archived
// incidents hide unless toggled (matching the TUI's `a`), and a fuzzy
// query ranks over title, id, systems, and tags — the same haystack the
// TUI filter searches.

import { score } from "./fuzzy";
import type { Workspace } from "../types";

export function filterWorkspaces(
  workspaces: readonly Workspace[],
  filter: string,
  showArchived: boolean,
): Workspace[] {
  const candidates = workspaces.filter((w) => showArchived || !w.archived);
  if (filter === "") {
    return candidates;
  }
  return candidates
    .map((workspace) => ({
      workspace,
      rank: score(
        filter,
        `${workspace.title} ${workspace.id} ${workspace.systems.join(" ")} ${workspace.tags.join(" ")}`,
      ),
    }))
    .filter(
      (entry): entry is { workspace: Workspace; rank: number } => entry.rank !== null,
    )
    .sort((a, b) => b.rank - a.rank)
    .map((entry) => entry.workspace);
}
