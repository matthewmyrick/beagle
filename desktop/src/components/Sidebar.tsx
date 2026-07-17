// The incident list: severity badge, title, status line per workspace.
// Archived rows render dimmed, matching the TUI.

import type { JSX } from "react";

import type { RefObject } from "react";

import { formatCreated, severityColor, statusGlyph } from "../lib/format";
import type { Workspace } from "../types";

interface SidebarProps {
  workspaces: Workspace[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  filter: string;
  onFilterChange: (value: string) => void;
  filterRef: RefObject<HTMLInputElement | null>;
  hiddenArchived: number;
  onShowArchived: () => void;
}

export function Sidebar({
  workspaces,
  selectedId,
  onSelect,
  filter,
  onFilterChange,
  filterRef,
  hiddenArchived,
  onShowArchived,
}: SidebarProps): JSX.Element {
  return (
    <nav className="sidebar" aria-label="Incidents">
      <h2 className="sidebar-heading">Incidents ({workspaces.length})</h2>
      <input
        ref={filterRef}
        className="filter-input"
        type="search"
        placeholder="filter…  ( / )"
        value={filter}
        onChange={(event) => {
          onFilterChange(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            onFilterChange("");
            event.currentTarget.blur();
          }
          if (event.key === "Enter") {
            event.currentTarget.blur();
          }
        }}
      />
      <ul className="sidebar-list">
        {workspaces.map((workspace) => (
          <SidebarRow
            key={workspace.id}
            workspace={workspace}
            selected={workspace.id === selectedId}
            onSelect={onSelect}
          />
        ))}
      </ul>
      {hiddenArchived > 0 ? (
        <button type="button" className="sidebar-footnote" onClick={onShowArchived}>
          {hiddenArchived} archived hidden — show
        </button>
      ) : null}
    </nav>
  );
}

interface SidebarRowProps {
  workspace: Workspace;
  selected: boolean;
  onSelect: (id: string) => void;
}

function SidebarRow({ workspace, selected, onSelect }: SidebarRowProps): JSX.Element {
  const classes = [
    "sidebar-row",
    selected ? "selected" : "",
    workspace.archived ? "archived" : "",
  ]
    .filter((c) => c !== "")
    .join(" ");
  return (
    <li>
      <button
        type="button"
        className={classes}
        onClick={() => {
          onSelect(workspace.id);
        }}
      >
        <span
          className="severity-badge"
          style={{ backgroundColor: severityColor(workspace.severity) }}
        >
          {workspace.severity.slice(0, 4).toUpperCase()}
        </span>
        <span className="row-title">{workspace.title}</span>
        <span className="row-detail">
          <span
            className={
              workspace.status === "investigating" ? "status-glyph pulse" : "status-glyph"
            }
          >
            {statusGlyph(workspace.status)}
          </span>{" "}
          {workspace.status}
          {workspace.archived ? " · archived" : ""} · {formatCreated(workspace.created)}
        </span>
      </button>
    </li>
  );
}
