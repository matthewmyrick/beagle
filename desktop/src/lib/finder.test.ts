import { describe, expect, it } from "vitest";

import { rankCorpus, toRuns } from "./finder";
import type { CorpusLine } from "./finder";

function line(overrides: Partial<CorpusLine>): CorpusLine {
  return {
    id: "rca-0",
    title: "Payments",
    file: "summary.md",
    line: 0,
    text: "something",
    ...overrides,
  };
}

describe("rankCorpus", () => {
  const corpus = [
    line({ text: "checkout requests time out" }),
    line({ id: "ledger", text: "redis pool exhausted", file: "notes.md", line: 4 }),
    line({ text: "unrelated chatter" }),
  ];

  it("matches nothing on an empty query — the popup opens quiet", () => {
    expect(rankCorpus(corpus, "")).toEqual([]);
  });

  it("ranks fuzzy matches and drops non-matches", () => {
    const hits = rankCorpus(corpus, "redispool");
    expect(hits).toHaveLength(1);
    expect(hits[0]?.entry.id).toBe("ledger");
    expect(hits[0]?.entry.line).toBe(4);
    expect(hits[0]?.positions).toHaveLength("redispool".length);
  });

  it("caps the result list", () => {
    const big = Array.from({ length: 500 }, (_, i) =>
      line({ text: `entry number ${String(i)}` }),
    );
    expect(rankCorpus(big, "entry").length).toBeLessThanOrEqual(100);
  });
});

describe("toRuns", () => {
  it("groups adjacent matched chars into single runs", () => {
    expect(toRuns("redis pool", [0, 1, 2, 3, 4, 6, 7, 8, 9])).toEqual([
      { text: "redis", matched: true },
      { text: " ", matched: false },
      { text: "pool", matched: true },
    ]);
  });

  it("returns one unmatched run with no positions", () => {
    expect(toRuns("plain", [])).toEqual([{ text: "plain", matched: false }]);
  });
});
