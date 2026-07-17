// Render test: rows appear, selection fires, archived rows are marked.
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { createRef } from "react";

import type { Workspace } from "../types";
import { Sidebar } from "./Sidebar";

const filterProps = {
  filter: "",
  onFilterChange: (): undefined => undefined,
  filterRef: createRef<HTMLInputElement>(),
  hiddenArchived: 0,
  onShowArchived: (): undefined => undefined,
};

function workspace(overrides: Partial<Workspace>): Workspace {
  return {
    id: "rca-0",
    title: "Payments latency",
    severity: "high",
    status: "review",
    created: "2026-07-15T14:32:00Z",
    systems: ["payments-api"],
    tags: [],
    prs: [],
    archived: false,
    ...overrides,
  };
}

afterEach(cleanup);

describe("Sidebar", () => {
  it("renders one row per workspace and reports clicks", () => {
    const onSelect = vi.fn();
    render(
      <Sidebar
        workspaces={[
          workspace({ id: "a", title: "First" }),
          workspace({ id: "b", title: "Second" }),
        ]}
        selectedId="a"
        onSelect={onSelect}
        {...filterProps}
      />,
    );
    expect(screen.getByText("First")).toBeDefined();
    fireEvent.click(screen.getByText("Second"));
    expect(onSelect).toHaveBeenCalledWith("b");
  });

  it("marks archived rows", () => {
    render(
      <Sidebar
        workspaces={[workspace({ id: "old", archived: true })]}
        selectedId={null}
        onSelect={() => undefined}
        {...filterProps}
      />,
    );
    expect(screen.getByText(/archived/)).toBeDefined();
  });
});

describe("archived footnote", () => {
  it("is a button that reveals the archive", () => {
    const onShowArchived = vi.fn();
    render(
      <Sidebar
        workspaces={[]}
        selectedId={null}
        onSelect={() => undefined}
        {...filterProps}
        hiddenArchived={2}
        onShowArchived={onShowArchived}
      />,
    );
    fireEvent.click(screen.getByText(/2 archived hidden/));
    expect(onShowArchived).toHaveBeenCalled();
  });
});
