import { describe, expect, it } from "vitest";

import { rankCorpus } from "./finder";
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

  it("lists the corpus unranked on an empty query", () => {
    expect(rankCorpus(corpus, "")).toHaveLength(3);
  });

  it("ranks fuzzy matches and drops non-matches", () => {
    const hits = rankCorpus(corpus, "redispool");
    expect(hits).toHaveLength(1);
    expect(hits[0]?.id).toBe("ledger");
    expect(hits[0]?.line).toBe(4);
  });

  it("caps the result list", () => {
    const big = Array.from({ length: 500 }, (_, i) =>
      line({ text: `entry number ${String(i)}` }),
    );
    expect(rankCorpus(big, "entry").length).toBeLessThanOrEqual(100);
  });
});
