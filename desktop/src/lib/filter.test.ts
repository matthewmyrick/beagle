import { describe, expect, it } from "vitest";

import { filterWorkspaces } from "./filter";
import type { Workspace } from "../types";

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

describe("filterWorkspaces", () => {
  const all = [
    workspace({ id: "payments", title: "Payments latency" }),
    workspace({ id: "ledger", title: "Ledger export stuck", systems: ["postgres"] }),
    workspace({ id: "old-cron", title: "Cron drift", archived: true }),
  ];

  it("hides archived incidents until toggled, like the TUI", () => {
    expect(filterWorkspaces(all, "", false).map((w) => w.id)).toEqual([
      "payments",
      "ledger",
    ]);
    expect(filterWorkspaces(all, "", true)).toHaveLength(3);
  });

  it("ranks fuzzy matches over title, id, and systems", () => {
    const hits = filterWorkspaces(all, "postgres", true);
    expect(hits.map((w) => w.id)).toEqual(["ledger"]);
  });

  it("drops non-matches entirely", () => {
    expect(filterWorkspaces(all, "zzz", true)).toEqual([]);
  });

  it("still finds archived incidents when they are shown", () => {
    expect(filterWorkspaces(all, "cron", true).map((w) => w.id)).toEqual(["old-cron"]);
    expect(filterWorkspaces(all, "cron", false)).toEqual([]);
  });
});
