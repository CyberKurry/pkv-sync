import { describe, expect, it } from "vitest";
import {
  diffLineClass,
  diffRestoreTargets,
  diffTitle
} from "../../src/ui/diff-modal";

describe("diff modal helpers", () => {
  it("maps parsed diff line kinds to stable CSS classes", () => {
    expect(diffLineClass("add")).toBe("pkvsync-diff-add");
    expect(diffLineClass("del")).toBe("pkvsync-diff-del");
    expect(diffLineClass("hunk")).toBe("pkvsync-diff-hunk");
    expect(diffLineClass("meta")).toBe("pkvsync-diff-meta");
    expect(diffLineClass("context")).toBe("pkvsync-diff-context");
  });

  it("uses short commit ids in the title", () => {
    expect(diffTitle("notes/today.md", "1234567890", "abcdef1234")).toBe(
      "notes/today.md 1234567..abcdef1"
    );
  });

  it("does not offer restore-right for deleted target commits", () => {
    expect(
      diffRestoreTargets(
        { from: "1234567890", to: "abcdef1234" },
        { allowRestoreRight: false }
      )
    ).toEqual({
      left: "1234567890"
    });
  });
});
