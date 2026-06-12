import { describe, expect, it } from "vitest";
import { wordDiff } from "../../src/sync/unified-diff";

describe("wordDiff", () => {
  it("marks only the changed word in a single-word edit", () => {
    const { leftSegments, rightSegments } = wordDiff(
      "the quick brown fox",
      "the slow brown fox"
    );
    expect(leftSegments).toEqual([
      { text: "the ", changed: false },
      { text: "quick", changed: true },
      { text: " brown fox", changed: false }
    ]);
    expect(rightSegments).toEqual([
      { text: "the ", changed: false },
      { text: "slow", changed: true },
      { text: " brown fox", changed: false }
    ]);
  });

  it("returns single unchanged segments for identical lines", () => {
    const { leftSegments, rightSegments } = wordDiff("same line", "same line");
    expect(leftSegments).toEqual([{ text: "same line", changed: false }]);
    expect(rightSegments).toEqual([{ text: "same line", changed: false }]);
  });

  it("falls back to whole-line change when most tokens differ", () => {
    const { leftSegments, rightSegments } = wordDiff(
      "alpha beta gamma delta",
      "one two three four"
    );
    expect(leftSegments).toEqual([
      { text: "alpha beta gamma delta", changed: true }
    ]);
    expect(rightSegments).toEqual([
      { text: "one two three four", changed: true }
    ]);
  });

  it("diffs CJK text per character", () => {
    const { leftSegments, rightSegments } = wordDiff("数据传输类", "数据转移类");
    expect(leftSegments).toEqual([
      { text: "数据", changed: false },
      { text: "传输", changed: true },
      { text: "类", changed: false }
    ]);
    expect(rightSegments).toEqual([
      { text: "数据", changed: false },
      { text: "转移", changed: true },
      { text: "类", changed: false }
    ]);
  });

  it("handles empty sides", () => {
    expect(wordDiff("", "new").rightSegments).toEqual([
      { text: "new", changed: true }
    ]);
    expect(wordDiff("", "").leftSegments).toEqual([]);
    expect(wordDiff("", "").rightSegments).toEqual([]);
  });

  it("treats whitespace-only changes as changes", () => {
    const { leftSegments } = wordDiff("a  b", "a b");
    expect(leftSegments.some((s) => s.changed)).toBe(true);
  });

  it("merges adjacent segments of the same kind", () => {
    const { rightSegments } = wordDiff("a b c", "a x y c");
    expect(rightSegments).toEqual([
      { text: "a ", changed: false },
      { text: "x y", changed: true },
      { text: " c", changed: false }
    ]);
  });
});
