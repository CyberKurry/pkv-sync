import { describe, expect, it } from "vitest";
import {
  lineDiffSideBySide,
  parseUnifiedDiffSideBySide
} from "../../src/sync/unified-diff";

describe("parseUnifiedDiffSideBySide", () => {
  it("pairs adjacent deleted and added lines for side-by-side rendering", () => {
    const rows = parseUnifiedDiffSideBySide(
      [
        "--- c1",
        "+++ c2",
        "@@ -4,3 +4,3 @@",
        " keep",
        "-old title",
        "+new title",
        " tail"
      ].join("\n")
    );

    expect(rows).toMatchObject([
      { kind: "meta", text: "--- c1" },
      { kind: "meta", text: "+++ c2" },
      { kind: "hunk", text: "@@ -4,3 +4,3 @@" },
      {
        kind: "context",
        leftLine: 4,
        rightLine: 4,
        leftText: "keep",
        rightText: "keep"
      },
      {
        kind: "modify",
        leftLine: 5,
        rightLine: 5,
        leftText: "old title",
        rightText: "new title"
      },
      {
        kind: "context",
        leftLine: 6,
        rightLine: 6,
        leftText: "tail",
        rightText: "tail"
      }
    ]);
  });

  it("pairs grouped deleted and added blocks line by line", () => {
    const rows = parseUnifiedDiffSideBySide(
      [
        "@@ -1,4 +1,4 @@",
        "-old title",
        "-old subtitle",
        "+new title",
        "+new subtitle"
      ].join("\n")
    );

    expect(rows.slice(1)).toMatchObject([
      {
        kind: "modify",
        leftLine: 1,
        rightLine: 1,
        leftText: "old title",
        rightText: "new title"
      },
      {
        kind: "modify",
        leftLine: 2,
        rightLine: 2,
        leftText: "old subtitle",
        rightText: "new subtitle"
      }
    ]);
  });

  it("builds side-by-side line diffs without front-inserting traceback operations", () => {
    const left = Array.from({ length: 80 }, (_, index) => `line ${index}`).join("\n");
    const right = Array.from({ length: 80 }, (_, index) =>
      index % 5 === 0 ? `changed ${index}` : `line ${index}`
    ).join("\n");
    const originalUnshift = Array.prototype.unshift;
    let unshiftCalls = 0;
    let result: ReturnType<typeof lineDiffSideBySide> | null = null;

    Object.defineProperty(Array.prototype, "unshift", {
      configurable: true,
      writable: true,
      value: function patchedUnshift(this: unknown[], ...items: unknown[]): number {
        unshiftCalls += 1;
        return originalUnshift.apply(this, items);
      }
    });

    try {
      result = lineDiffSideBySide(left, right);
    } finally {
      Object.defineProperty(Array.prototype, "unshift", {
        configurable: true,
        writable: true,
        value: originalUnshift
      });
    }

    expect(unshiftCalls).toBe(0);
    expect(result?.truncated).toBe(false);
    expect(result?.rows.some((row) => row.kind !== "context")).toBe(true);
  });
});
