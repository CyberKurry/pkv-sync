import { describe, expect, it, vi } from "vitest";
import {
  DiffModal,
  type DiffModalLabels,
  commitOptionLabel,
  diffRestoreTargets,
  diffTitle,
  uniqueCommits
} from "../../src/ui/diff-modal";
import type { CommitSummary, UnifiedDiff } from "../../src/api/types";

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

describe("DiffModal async rendering", () => {
  it("does not render a completed diff after the modal is closed", async () => {
    const pendingDiff = deferred<UnifiedDiff>();
    const modal = new DiffModal({} as never, {
      api: {
        diff: vi.fn(() => pendingDiff.promise),
        fileHistory: vi.fn(async () => [])
      } as never,
      vaultId: "vault-1",
      path: "note.md",
      to: "c1",
      labels
    });
    const contentEl = modal.contentEl as unknown as {
      empty: ReturnType<typeof vi.fn>;
      createDiv: ReturnType<typeof vi.fn>;
    };

    modal.open();
    modal.close();
    contentEl.empty.mockClear();
    contentEl.createDiv.mockClear();

    pendingDiff.resolve({
      from: "c0",
      to: "c1",
      path: "note.md",
      binary: true,
      truncated: false,
      patch: ""
    });
    await flushPromises();

    expect(contentEl.empty).not.toHaveBeenCalled();
    expect(contentEl.createDiv).not.toHaveBeenCalled();
  });
});

const labels: DiffModalLabels = {
  diffTitle: "Diff",
  diffBinary: "Binary file",
  diffTruncated: "Diff truncated",
  diffFrom: "From",
  diffTo: "To",
  diffPrevious: "Previous",
  diffRestoreLeft: "Restore left",
  diffRestoreRight: "Restore right",
  historyRetry: "Retry"
};

function deferred<T>(): {
  promise: Promise<T>;
  resolve(value: T): void;
  reject(error: unknown): void;
} {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function flushPromises(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}
