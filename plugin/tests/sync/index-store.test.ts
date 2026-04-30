import { describe, expect, it } from "vitest";
import {
  deletedFiles,
  markDeleted,
  markSynced,
  normalizeIndex,
  pendingFiles
} from "../../src/sync/index-store";
import type { LocalFileSnapshot } from "../../src/sync/types";

const f = (path: string, hash: string): LocalFileSnapshot => ({
  path,
  hash,
  size: 1,
  kind: "text",
  content: "x"
});

describe("index-store", () => {
  it("normalizes bad raw data", () => {
    expect(normalizeIndex(null)).toEqual({
      lastSyncedCommit: null,
      files: {}
    });
  });

  it("markSynced stores hashes", () => {
    const idx = markSynced(
      { lastSyncedCommit: null, files: {} },
      "c1",
      [f("a.md", "h1")]
    );
    expect(idx.lastSyncedCommit).toBe("c1");
    expect(idx.files["a.md"].lastSyncedHash).toBe("h1");
  });

  it("pendingFiles returns changed files only", () => {
    const idx = markSynced(
      { lastSyncedCommit: null, files: {} },
      "c1",
      [f("a.md", "h1")]
    );
    expect(
      pendingFiles(idx, [f("a.md", "h1"), f("b.md", "h2")]).map(
        (x) => x.path
      )
    ).toEqual(["b.md"]);
  });

  it("deletedFiles returns indexed paths missing now", () => {
    const idx = markSynced(
      { lastSyncedCommit: null, files: {} },
      "c1",
      [f("a.md", "h1"), f("b.md", "h2")]
    );
    expect(deletedFiles(idx, [f("a.md", "h1")])).toEqual(["b.md"]);
  });

  it("markDeleted removes paths", () => {
    const idx = markSynced(
      { lastSyncedCommit: null, files: {} },
      "c1",
      [f("a.md", "h1")]
    );
    expect(markDeleted(idx, "c2", ["a.md"]).files["a.md"]).toBeUndefined();
  });
});
