import { describe, expect, it } from "vitest";
import { historyEntryView, shortCommit } from "../../src/ui/history-modal";
import type { CommitSummary } from "../../src/api/types";

function commit(overrides: Partial<CommitSummary> = {}): CommitSummary {
  return {
    commit: "1234567890abcdef",
    parent: "abcdef1234567890",
    message: "sync: Laptop\n\nUpdated note",
    timestamp: 1_700_000_000,
    author_device: "Laptop",
    change_type: "modified",
    ...overrides
  };
}

describe("history modal helpers", () => {
  it("shortens commit ids for compact rows", () => {
    expect(shortCommit("1234567890abcdef")).toBe("1234567");
  });

  it("builds readable history row data and hides restore for deleted entries", () => {
    expect(historyEntryView(commit()).canRestore).toBe(true);
    expect(historyEntryView(commit()).canRollback).toBe(true);

    const view = historyEntryView(
      commit({
        message: "",
        author_device: null,
        change_type: "deleted"
      })
    );

    expect(view.title).toBe("1234567");
    expect(view.device).toBe("Unknown device");
    expect(view.canRestore).toBe(false);
    expect(view.canRollback).toBe(false);
  });

  it("exposes rollback separately from file restore", () => {
    const view = historyEntryView(commit());

    expect(view.canRestore).toBe(true);
    expect(view.canRollback).toBe(true);
  });
});
