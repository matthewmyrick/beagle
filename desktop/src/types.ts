// The IPC contract with the Rust side. Mirrors
// desktop/src-tauri/src/dto.rs — keep the two in sync when either changes.

export type Severity = "critical" | "high" | "medium" | "low" | "info";

export type Status = "investigating" | "review" | "agent" | "final-review" | "finished";

/** One workspace row for the sidebar. */
export interface Workspace {
  id: string;
  title: string;
  severity: Severity;
  status: Status;
  /** RFC 3339 creation timestamp. */
  created: string;
  systems: string[];
  tags: string[];
  prs: string[];
  archived: boolean;
}

/** A workspace directory that exists on disk but could not load. */
export interface Broken {
  dir_name: string;
  reason: string;
}

/** Everything `list_workspaces` returns in one call. */
export interface Listing {
  root: string;
  workspaces: Workspace[];
  broken: Broken[];
  warnings: string[];
}
