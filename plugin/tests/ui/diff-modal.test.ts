import { describe, expect, it } from "vitest";
import {
  commitOptionLabel,
  diffRestoreTargets,
  diffTitle,
  uniqueCommits
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

  it("keeps unique commits in first-seen order", () => {
    expect(uniqueCommits(["c2", null, "c1", "c2", undefined, "c3"])).toEqual([
      "c2",
      "c1",
      "c3"
    ]);
  });
});
