import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { SyncEngine, type IndexPersistence } from "../../src/sync/engine";
import type { LocalFileSnapshot, LocalIndex } from "../../src/sync/types";

class FakeVault {
  constructor(public files: LocalFileSnapshot[]) {}

  async scan(): Promise<LocalFileSnapshot[]> {
    return this.files;
  }
}

class FakeIndex implements IndexPersistence {
  saved: LocalIndex | null = null;

  constructor(public idx: LocalIndex) {}

  async loadIndex(): Promise<LocalIndex> {
    return this.idx;
  }

  async saveIndex(index: LocalIndex): Promise<void> {
    this.saved = index;
    this.idx = index;
  }

  async updateIndex(
    updater: (index: LocalIndex) => LocalIndex | Promise<LocalIndex>
  ): Promise<void> {
    const next = await updater(this.idx);
    this.saved = next;
    this.idx = next;
  }
}

describe("SyncEngine push", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("pushes changed text and updates index", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: null, files: {} });
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: null,
        changed_since: false
      }),
      pull: vi.fn(),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({ new_commit: "c1", files_changed: 1 }),
      downloadBlob: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: new FakeVault([
        {
          path: "a.md",
          hash: "h",
          size: 2,
          kind: "text",
          content: "hi"
        }
      ]) as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    expect(api.push).toHaveBeenCalledWith("v", null, [
      { kind: "text", path: "a.md", content: "hi" }
    ], "d");
    expect(idx.saved?.lastSyncedCommit).toBe("c1");
    expect(idx.saved?.files["a.md"].lastSyncedHash).toBe("h");
  });

  it("fetches vault settings and filters hidden push candidates with cached allowlist fallback", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: null, files: {} });
    const getVaultSettings = vi
      .fn()
      .mockResolvedValueOnce({
        extra_sync_globs: [".obsidian/themes/**"]
      })
      .mockRejectedValueOnce(new Error("settings unavailable"));
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const api = {
      api: { getVaultSettings },
      state: vi.fn().mockResolvedValue({
        current_head: null,
        changed_since: false
      }),
      pull: vi.fn(),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi
        .fn()
        .mockResolvedValueOnce({ new_commit: "c1", files_changed: 2 })
        .mockResolvedValueOnce({ new_commit: "c2", files_changed: 1 }),
      downloadBlob: vi.fn()
    };
    const vault = new FakeVault([
      {
        path: "notes/a.md",
        hash: "h1",
        size: 2,
        kind: "text",
        content: "hi"
      },
      {
        path: ".obsidian/themes/custom.css",
        hash: "h2",
        size: 6,
        kind: "text",
        content: "theme"
      },
      {
        path: ".obsidian/plugins/foo/main.js",
        hash: "h3",
        size: 6,
        kind: "text",
        content: "plugin"
      }
    ]);
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md", "css", "js"]),
      extraExcludeGlobs: ["notes/private/**"],
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();
    vault.files = [
      {
        path: ".obsidian/themes/other.css",
        hash: "h4",
        size: 5,
        kind: "text",
        content: "other"
      },
      {
        path: ".obsidian/plugins/bar/main.js",
        hash: "h5",
        size: 6,
        kind: "text",
        content: "plugin"
      },
      {
        path: "notes/private/secret.md",
        hash: "h6",
        size: 6,
        kind: "text",
        content: "secret"
      }
    ];
    await engine.syncNow();

    expect(getVaultSettings).toHaveBeenCalledTimes(2);
    expect(getVaultSettings).toHaveBeenCalledWith("v");
    expect(warn).toHaveBeenCalledWith(
      "[pkv-sync] failed to refresh vault settings; using cached settings:",
      expect.any(Error)
    );
    expect(api.push).toHaveBeenNthCalledWith(1, "v", null, [
      { kind: "text", path: "notes/a.md", content: "hi" },
      {
        kind: "text",
        path: ".obsidian/themes/custom.css",
        content: "theme"
      }
    ], "d");
    expect(api.push).toHaveBeenNthCalledWith(2, "v", "c1", [
      {
        kind: "text",
        path: ".obsidian/themes/other.css",
        content: "other"
      },
      { kind: "delete", path: "notes/a.md" },
      { kind: "delete", path: ".obsidian/themes/custom.css" }
    ], "d");
  });

  it("notifies after a successful sync", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: null, files: {} });
    const onSyncSuccess = vi.fn();
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: null,
        changed_since: false
      }),
      pull: vi.fn(),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({ new_commit: "c1", files_changed: 1 }),
      downloadBlob: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: new FakeVault([
        {
          path: "a.md",
          hash: "h",
          size: 2,
          kind: "text",
          content: "hi"
        }
      ]) as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn(),
      onSyncSuccess
    });

    await engine.syncNow();

    expect(onSyncSuccess).toHaveBeenCalledTimes(1);
  });

  it("uploads missing blobs before pushing manifest changes", async () => {
    const bytes = new Uint8Array([1, 2, 3]).buffer;
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: "c0",
        changed_since: false
      }),
      pull: vi.fn(),
      uploadCheck: vi.fn().mockResolvedValue({ missing: ["blob-hash"] }),
      uploadBlob: vi.fn().mockResolvedValue(undefined),
      push: vi.fn().mockResolvedValue({ new_commit: "c1", files_changed: 1 }),
      downloadBlob: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: new FakeVault([
        {
          path: "image.png",
          hash: "blob-hash",
          size: 3,
          kind: "blob",
          bytes
        }
      ]) as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    expect(api.uploadCheck).toHaveBeenCalledWith("v", ["blob-hash"]);
    expect(api.uploadBlob).toHaveBeenCalledWith("v", "blob-hash", bytes);
    expect(api.push).toHaveBeenCalledWith("v", "c0", [
      {
        kind: "blob",
        path: "image.png",
        blob_hash: "blob-hash",
        size: 3,
        mime: "image/png"
      }
    ], "d");
  });

  it("flushOnUnload pushes pending changes immediately", async () => {
    vi.stubGlobal("window", globalThis);
    const idx = new FakeIndex({ lastSyncedCommit: null, files: {} });
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: null,
        changed_since: false
      }),
      pull: vi.fn(),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({ new_commit: "c1", files_changed: 1 }),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: new FakeVault([
        {
          path: "pending.md",
          hash: "h",
          size: 7,
          kind: "text",
          content: "pending"
        }
      ]) as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.flushOnUnload(1500);

    expect(api.push).toHaveBeenCalledWith("v", null, [
      { kind: "text", path: "pending.md", content: "pending" }
    ], "d");
  });

  it("pulls latest head and retries once after head_mismatch", async () => {
    const idx = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "a.md": {
          lastSyncedHash: "old",
          lastSyncedAt: 1,
          kind: "text",
          size: 3
        }
      }
    });
    const api = {
      state: vi
        .fn()
        .mockResolvedValueOnce({
          current_head: "c0",
          changed_since: false
        })
        .mockResolvedValueOnce({
          current_head: "c1",
          changed_since: true
        }),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c1",
        added: [],
        modified: [],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi
        .fn()
        .mockRejectedValueOnce(
          new ApiError(409, "head_mismatch", "current head is c1")
        )
        .mockResolvedValueOnce({ new_commit: "c2", files_changed: 1 }),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: new FakeVault([
        {
          path: "a.md",
          hash: "new",
          size: 3,
          kind: "text",
          content: "new"
        }
      ]) as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    expect(api.push).toHaveBeenNthCalledWith(1, "v", "c0", [
      { kind: "text", path: "a.md", content: "new" }
    ], "d");
    expect(api.push).toHaveBeenNthCalledWith(2, "v", "c1", [
      { kind: "text", path: "a.md", content: "new" }
    ], "d");
    expect(idx.saved?.lastSyncedCommit).toBe("c2");
  });
});
