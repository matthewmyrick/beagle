import { describe, expect, it } from "vitest";

import { stripLeadingHeading, stripScaffoldHint } from "./text";

describe("stripLeadingHeading", () => {
  it("drops a leading # heading and the blank line after it", () => {
    expect(stripLeadingHeading("# Summary\n\nThe body.")).toBe("The body.");
  });

  it("leaves content with no leading heading unchanged", () => {
    expect(stripLeadingHeading("Just prose.")).toBe("Just prose.");
  });

  it("does not strip a deeper heading", () => {
    expect(stripLeadingHeading("## Sub\n\nx")).toBe("## Sub\n\nx");
  });
});

describe("stripScaffoldHint", () => {
  it("removes the leading `> _hint_` blockquote beagle new writes", () => {
    expect(stripScaffoldHint("> _What broke, in three sentences._\n\nReal content")).toBe(
      "Real content",
    );
  });

  it("leaves real blockquotes alone", () => {
    expect(stripScaffoldHint("> a real quote\n")).toBe("> a real quote\n");
  });
});
