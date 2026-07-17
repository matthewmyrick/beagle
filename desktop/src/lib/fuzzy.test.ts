import { describe, expect, it } from "vitest";

import { score } from "./fuzzy";

describe("score", () => {
  it("requires the needle's characters in order", () => {
    expect(score("abc", "a-b-c")).not.toBeNull();
    expect(score("cba", "a-b-c")).toBeNull();
  });

  it("matches everything with 0 on an empty needle", () => {
    expect(score("", "anything")).toBe(0);
  });

  it("is case-insensitive", () => {
    expect(score("RCA", "rca-0")).not.toBeNull();
  });

  it("ranks a tight substring above the same letters scattered", () => {
    const tight = score("payapi", "payments-api");
    const scattered = score("payapi", "p-a-y-something-a-p-i-else");
    expect(tight).not.toBeNull();
    expect(scattered).not.toBeNull();
    if (tight !== null && scattered !== null) {
      expect(tight).toBeGreaterThan(scattered);
    }
  });

  it("rewards word boundaries like the TUI scorer", () => {
    const boundary = score("re", "redis-pool");
    const middle = score("re", "chore");
    expect(boundary).not.toBeNull();
    expect(middle).not.toBeNull();
    if (boundary !== null && middle !== null) {
      expect(boundary).toBeGreaterThan(middle);
    }
  });
});
