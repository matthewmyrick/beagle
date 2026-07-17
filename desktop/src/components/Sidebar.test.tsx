// Render test: rows appear, selection fires, archived rows are marked.
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Workspace } from "../types";
import { Sidebar } from "./Sidebar";

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
      />,
    );
    expect(screen.getByText(/archived/)).toBeDefined();
  });
});
