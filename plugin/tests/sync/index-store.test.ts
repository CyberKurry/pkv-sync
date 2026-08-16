import { describe, expect, it } from "vitest";
import {
  markBatch,
  markDeleted,
  markFilesDeleted,
  markFilesSynced,
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

const ix = (hash: string) => ({
  lastSyncedHash: hash,
  lastSyncedAt: 1,
  kind: "text" as const,
  size: 1
});

describe("index-store", () => {
  const hasOwn = (value: object, key: string): boolean =>
    Object.prototype.hasOwnProperty.call(value, key);

  it("normalizes bad raw data", () => {
    expect(normalizeIndex(null)).toEqual({
      lastSyncedCommit: null,
      files: {}
    });
  });

  it("normalizes files into a null-prototype map without dangerous keys", () => {
    const raw = JSON.parse(
      '{"lastSyncedCommit":"c1","files":{"safe.md":{"lastSyncedHash":"h1","lastSyncedAt":1,"kind":"text","size":1},"__proto__":{"polluted":true},"constructor":{"polluted":true},"prototype":{"polluted":true}}}'
    );

    const index = normalizeIndex(raw);

    expect(index.lastSyncedCommit).toBe("c1");
    expect(Object.getPrototypeOf(index.files)).toBeNull();
    expect(hasOwn(index.files, "safe.md")).toBe(true);
    expect(hasOwn(index.files, "__proto__")).toBe(false);
    expect(hasOwn(index.files, "constructor")).toBe(false);
    expect(hasOwn(index.files, "prototype")).toBe(false);
    expect("polluted" in index.files).toBe(false);
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

  it("markDeleted removes paths", () => {
    const idx = markSynced(
      { lastSyncedCommit: null, files: {} },
      "c1",
      [f("a.md", "h1")]
    );
    expect(markDeleted(idx, "c2", ["a.md"]).files["a.md"]).toBeUndefined();
  });

  it("markBatch equals markSynced then markDeleted in one call (advancing)", () => {
    const base = {
      lastSyncedCommit: "c0",
      files: {
        "keep.md": ix("h0"),
        "del.md": ix("hd")
      }
    };
    const upserts = [f("a.md", "h1"), f("b.md", "h2")];
    const deletes = ["del.md"];

    const batch = markBatch(base, "c1", upserts, deletes, true);
    const sequential = markSynced(
      markDeleted(base, "c1", deletes),
      "c1",
      upserts
    );

    // markBatch records one batch-wide Date.now(); the old pair recorded two
    // (one per call), so normalize timestamps before comparing structure.
    for (const index of [batch, sequential]) {
      for (const entry of Object.values(index.files)) entry.lastSyncedAt = 0;
    }
    expect(batch).toEqual(sequential);
    expect(batch.lastSyncedCommit).toBe("c1");
    expect(Object.keys(batch.files).sort()).toEqual(["a.md", "b.md", "keep.md"]);
  });

  it("markBatch does not advance the commit when advance=false", () => {
    const base = { lastSyncedCommit: "c0", files: {} };
    const next = markBatch(base, "c9", [f("a.md", "h1")], [], false);
    expect(next.lastSyncedCommit).toBe("c0");
  });

  it("markBatch is immutable with respect to its input index", () => {
    const base = {
      lastSyncedCommit: "c0",
      files: {
        "a.md": ix("h0"),
        "del.md": ix("hd")
      }
    };
    const before = JSON.stringify(base);
    markBatch(base, "c1", [f("a.md", "h1")], ["del.md"], true);
    expect(JSON.stringify(base)).toBe(before);
  });

  it("markFilesSynced and markFilesDeleted delegate to the shared helper", () => {
    const base = {
      lastSyncedCommit: "c0",
      files: {
        "a.md": ix("h0"),
        "del.md": ix("hd")
      }
    };
    const synced = markFilesSynced(base, [f("a.md", "h1")]);
    expect(synced.lastSyncedCommit).toBe("c0");
    expect(synced.files["a.md"].lastSyncedHash).toBe("h1");
    expect(markFilesDeleted(synced, ["del.md"]).files["del.md"]).toBeUndefined();
  });
});
