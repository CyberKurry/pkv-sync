import { describe, expect, it } from "vitest";
import { parseUnifiedDiff } from "../../src/sync/unified-diff";

describe("parseUnifiedDiff", () => {
  it("classifies metadata, hunks, added, deleted, and context lines", () => {
    const lines = parseUnifiedDiff(
      [
        "--- c1",
        "+++ c2",
        "@@ -1,2 +1,2 @@",
        " same",
        "-old",
        "+new"
      ].join("\n")
    );

    expect(lines.map((line) => line.kind)).toEqual([
      "meta",
      "meta",
      "hunk",
      "context",
      "del",
      "add"
    ]);
  });
});
