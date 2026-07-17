import { describe, expect, it } from "vitest";

import { nextTheme, resolveTheme } from "./theme";

describe("resolveTheme", () => {
  it("honors an explicit stored choice over the OS preference", () => {
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });

  it("falls back to the OS preference when storage is empty or junk", () => {
    expect(resolveTheme(null, true)).toBe("dark");
    expect(resolveTheme(null, false)).toBe("light");
    expect(resolveTheme("solarized", true)).toBe("dark");
    expect(resolveTheme("", false)).toBe("light");
  });
});

describe("nextTheme", () => {
  it("round-trips between the two themes", () => {
    expect(nextTheme("dark")).toBe("light");
    expect(nextTheme("light")).toBe("dark");
    expect(nextTheme(nextTheme("dark"))).toBe("dark");
  });
});
