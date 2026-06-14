import { describe, expect, it } from "vitest";
import {
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
});
