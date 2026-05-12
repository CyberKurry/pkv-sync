import { describe, expect, it } from "vitest";
import {
  commitOptionLabel,
  diffLineClass,
  diffRestoreTargets,
  diffTitle
} from "../../src/ui/diff-modal";
import type { CommitSummary } from "../../src/api/types";

function commit(overrides: Partial<CommitSummary> = {}): CommitSummary {
  return {
    commit: "1234567890abcdef",
    parent: "abcdef1234567890",
    message: "sync: Laptop\n\nUpdated note",
    timestamp: 0,
    author_device: "Laptop",
    change_type: "modified",
    ...overrides
  };
}

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

  it("labels commit options with the file history timestamp", () => {
    expect(
      commitOptionLabel(
        "1234567890abcdef",
        [commit()],
        "Asia/Shanghai"
      )
    ).toBe("1234567 - 1970-01-01 08:00:00");
  });
});
