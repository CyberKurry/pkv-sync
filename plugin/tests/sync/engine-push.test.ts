import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { SyncEngine, type IndexPersistence } from "../../src/sync/engine";
import { sha256Text } from "../../src/sync/hash";
import type { LocalFileSnapshot, LocalIndex } from "../../src/sync/types";
import type { SyncApi } from "../../src/api/sync-client";
import type { VaultAdapter } from "../../src/sync/vault-adapter";

class FakeVault {
  writes = new Map<string, string>();
  deletions: string[] = [];

  constructor(public files: LocalFileSnapshot[]) {}

  async scan(): Promise<LocalFileSnapshot[]> {
    return this.files;
  }

  exists(path: string): boolean {
    return this.files.some((file) => file.path === path);
  }

  async readText(path: string): Promise<string> {
    const file = this.files.find((entry) => entry.path === path);
    if (!file || file.kind !== "text") throw new Error(`not found: ${path}`);
    return file.content ?? "";
  }

  async writeText(path: string, content: string): Promise<void> {
    this.writes.set(path, content);
    const next: LocalFileSnapshot = {
      path,
      hash: await sha256Text(content),
      size: new TextEncoder().encode(content).byteLength,
      kind: "text",
      content
    };
    this.files = this.files.filter((file) => file.path !== path).concat(next);
  }

  async writeBinary(path: string, bytes: ArrayBuffer): Promise<void> {
    this.files = this.files
      .filter((file) => file.path !== path)
      .concat({
        path,
        hash: "blob-hash",
        size: bytes.byteLength,
        kind: "blob",
        bytes
      });
  }

  async delete(path: string): Promise<void> {
    this.deletions.push(path);
    this.files = this.files.filter((file) => file.path !== path);
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

function deferred(): {
  promise: Promise<void>;
  resolve: () => void;
  reject: (error: Error) => void;
} {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("SyncEngine push", () => {
  beforeEach(() => {
    vi.stubGlobal("window", globalThis);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it("pushes changed text and updates index", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: null, files: {} });
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: null,
        changed_since: false
      }),
      pull: vi.fn().mockResolvedValue({
        from: null,
        to: null,
        added: [],
        modified: [],
        deleted: []
      }),
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

  it("reuses the unchanged pull scan when pushing pending files", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([
      {
        path: "a.md",
        hash: "h",
        size: 2,
        kind: "text",
        content: "hi"
      }
    ]);
    const scan = vi.spyOn(vault, "scan");
    const api = {
      state: vi.fn(),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c0",
        added: [],
        modified: [],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({ new_commit: "c1", files_changed: 1 }),
      downloadBlob: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    // scanPending (step ①) + applyPull inside pullIfChanged (step ③)
    expect(scan).toHaveBeenCalledTimes(2);
    expect(api.push).toHaveBeenCalledWith("v", "c0", [
      { kind: "text", path: "a.md", content: "hi" }
    ], "d");
  });

  it("fetches vault settings and filters hidden push candidates with cached allowlist fallback", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: null, files: {} });
    const getVaultSettings = vi
      .fn()
      .mockResolvedValueOnce({
        extra_sync_globs: [".obsidian/themes/**"]
      })
      .mockRejectedValueOnce(new Error("settings unavailable"));
    const previousNodeEnv = process.env.NODE_ENV;
    process.env.NODE_ENV = "development";
    const debug = vi.spyOn(console, "debug").mockImplementation(() => undefined);
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: null,
        changed_since: false
      }),
      pull: vi.fn().mockResolvedValue({
        from: null,
        to: null,
        added: [],
        modified: [],
        deleted: []
      }),
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
      setStatus: vi.fn(),
      vaultSettingsReader: getVaultSettings
    });

    try {
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
      expect(debug).toHaveBeenCalledWith(
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
    } finally {
      process.env.NODE_ENV = previousNodeEnv;
      debug.mockRestore();
    }
  });

  it("uses an explicit vault settings reader for allowlisted push candidates", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: null, files: {} });
    const vaultSettingsReader = vi.fn().mockResolvedValue({
      extra_sync_globs: [".obsidian/themes/**"]
    });
    const api = {
      state: vi.fn(),
      pull: vi.fn().mockResolvedValue({
        from: null,
        to: null,
        added: [],
        modified: [],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({ new_commit: "c1", files_changed: 1 }),
      downloadBlob: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["css", "js"]),
      vault: new FakeVault([
        {
          path: ".obsidian/themes/custom.css",
          hash: "h1",
          size: 6,
          kind: "text",
          content: "theme"
        },
        {
          path: ".obsidian/plugins/foo/main.js",
          hash: "h2",
          size: 6,
          kind: "text",
          content: "plugin"
        }
      ]) as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn(),
      vaultSettingsReader
    });

    await engine.syncNow();

    expect(vaultSettingsReader).toHaveBeenCalledWith("v");
    expect(api.push).toHaveBeenCalledWith("v", null, [
      {
        kind: "text",
        path: ".obsidian/themes/custom.css",
        content: "theme"
      }
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
      pull: vi.fn().mockResolvedValue({
        from: null,
        to: null,
        added: [],
        modified: [],
        deleted: []
      }),
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
      pull: vi.fn().mockResolvedValue({
        from: null,
        to: null,
        added: [],
        modified: [],
        deleted: []
      }),
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

  it("starts missing blob uploads concurrently and pushes only after all uploads succeed", async () => {
    const bytes1 = new Uint8Array([1]).buffer;
    const bytes2 = new Uint8Array([2]).buffer;
    const bytes3 = new Uint8Array([3]).buffer;
    const bytes4 = new Uint8Array([4]).buffer;
    const uploads = new Map([
      ["h1", deferred()],
      ["h2", deferred()],
      ["h3", deferred()]
    ]);
    const uploadStarts: string[] = [];
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: "c0",
        changed_since: false
      }),
      pull: vi.fn().mockResolvedValue({
        from: null,
        to: null,
        added: [],
        modified: [],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: ["h1", "h2", "h3"] }),
      uploadBlob: vi.fn((vaultId: string, hash: string, bytes: ArrayBuffer) => {
        uploadStarts.push(hash);
        const upload = uploads.get(hash);
        if (!upload) throw new Error(`unexpected upload for ${hash}`);
        expect(vaultId).toBe("v");
        expect(bytes).toBe(hash === "h1" ? bytes1 : hash === "h2" ? bytes2 : bytes3);
        return upload.promise;
      }),
      push: vi.fn().mockResolvedValue({ new_commit: "c1", files_changed: 4 }),
      downloadBlob: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: new FakeVault([
        {
          path: "one.bin",
          hash: "h1",
          size: 1,
          kind: "blob",
          bytes: bytes1
        },
        {
          path: "two.bin",
          hash: "h2",
          size: 1,
          kind: "blob",
          bytes: bytes2
        },
        {
          path: "three.bin",
          hash: "h3",
          size: 1,
          kind: "blob",
          bytes: bytes3
        },
        {
          path: "already-uploaded.bin",
          hash: "h4",
          size: 1,
          kind: "blob",
          bytes: bytes4
        }
      ]) as unknown as VaultAdapter,
      api: api as unknown as SyncApi,
      index: idx,
      setStatus: vi.fn()
    });

    const sync = engine.syncNow();
    await vi.waitFor(() => {
      expect(uploadStarts).toEqual(["h1", "h2", "h3"]);
    });
    expect(api.push).not.toHaveBeenCalled();
    expect(api.uploadBlob).not.toHaveBeenCalledWith("v", "h4", bytes4);

    uploads.get("h1")?.resolve();
    await Promise.resolve();
    expect(api.push).not.toHaveBeenCalled();

    uploads.get("h2")?.resolve();
    uploads.get("h3")?.resolve();
    await sync;

    expect(api.push).toHaveBeenCalledWith("v", "c0", [
      {
        kind: "blob",
        path: "one.bin",
        blob_hash: "h1",
        size: 1,
        mime: undefined
      },
      {
        kind: "blob",
        path: "two.bin",
        blob_hash: "h2",
        size: 1,
        mime: undefined
      },
      {
        kind: "blob",
        path: "three.bin",
        blob_hash: "h3",
        size: 1,
        mime: undefined
      },
      {
        kind: "blob",
        path: "already-uploaded.bin",
        blob_hash: "h4",
        size: 1,
        mime: undefined
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
      pull: vi.fn().mockResolvedValue({
        from: null,
        to: null,
        added: [],
        modified: [],
        deleted: []
      }),
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

  it("clears unload timeout when sync finishes before the timeout", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("window", globalThis);
    const clearTimeoutSpy = vi.spyOn(globalThis, "clearTimeout");
    const idx = new FakeIndex({ lastSyncedCommit: null, files: {} });
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: null,
        changed_since: false
      }),
      pull: vi.fn().mockResolvedValue({
        from: null,
        to: null,
        added: [],
        modified: [],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn(),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: new FakeVault([]) as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.flushOnUnload(1500);

    expect(clearTimeoutSpy).toHaveBeenCalled();
  });

  // --- Task 2: push-first sync with merge outcome backflow ---

  it("does not advance head past an unseen merge commit", async () => {
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
    const vault = new FakeVault([
      {
        path: "a.md",
        hash: "new",
        size: 3,
        kind: "text",
        content: "new"
      }
    ]);
    const api = {
      state: vi.fn(),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c0",
        added: [],
        modified: [],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({
        new_commit: "c1",
        files_changed: 1,
        merge_outcomes: [{ path: "a.md", outcome: "merged" }]
      }),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    // Hard rule: head must NOT advance past an unseen merge commit
    expect(idx.saved?.lastSyncedCommit).toBe("c0");
    // But per-file hash must be updated to the pushed content hash
    expect(idx.saved?.files["a.md"]?.lastSyncedHash).toBe("new");
  });

  it("advances head immediately when all merge outcomes are clean", async () => {
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
    const vault = new FakeVault([
      {
        path: "a.md",
        hash: "new",
        size: 3,
        kind: "text",
        content: "new"
      }
    ]);
    const api = {
      state: vi.fn(),
      pull: vi.fn().mockImplementation((_vaultId: string, since: string | null) => {
        // When called after push advances head to c1, return "up to date"
        return Promise.resolve({
          from: since,
          to: since,
          added: [],
          modified: [],
          deleted: []
        });
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({
        new_commit: "c1",
        files_changed: 1,
        merge_outcomes: [{ path: "a.md", outcome: "clean" }]
      }),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    // All-clean: head advances immediately (byte-identical to pre-push-first behavior)
    expect(idx.saved?.lastSyncedCommit).toBe("c1");
    expect(idx.saved?.files["a.md"]?.lastSyncedHash).toBe("new");
  });

  it("advances head immediately when merge_outcomes field is absent", async () => {
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
    const vault = new FakeVault([
      {
        path: "a.md",
        hash: "new",
        size: 3,
        kind: "text",
        content: "new"
      }
    ]);
    const api = {
      state: vi.fn(),
      pull: vi.fn().mockImplementation((_vaultId: string, since: string | null) => {
        return Promise.resolve({
          from: since,
          to: since,
          added: [],
          modified: [],
          deleted: []
        });
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({
        new_commit: "c1",
        files_changed: 1
        // merge_outcomes intentionally absent — old server compatibility
      }),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    expect(idx.saved?.lastSyncedCommit).toBe("c1");
    expect(idx.saved?.files["a.md"]?.lastSyncedHash).toBe("new");
  });

  it("backflow pull overwrites local and advances head after merged push", async () => {
    const pushedHash = await sha256Text("new");
    const mergedContent = "<<<< local\nnew\n====\nremote\n>>>> remote";
    const mergedHash = await sha256Text(mergedContent);
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
    const vault = new FakeVault([
      {
        path: "a.md",
        hash: pushedHash,
        size: 3,
        kind: "text",
        content: "new"
      }
    ]);
    const api = {
      state: vi.fn(),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c1",
        added: [],
        modified: [
          {
            path: "a.md",
            file_type: "text",
            size: mergedContent.length,
            content_inline: mergedContent
          }
        ],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({
        new_commit: "c1",
        files_changed: 1,
        merge_outcomes: [{ path: "a.md", outcome: "merged" }]
      }),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    // Push step: head stays at c0, per-file hash updated to pushed content hash
    // Then pull step: merged content arrives, local hash == pushed hash => NOT dirty => clean overwrite
    // After pull, head advances to c1
    expect(idx.saved?.lastSyncedCommit).toBe("c1");
    expect(idx.saved?.files["a.md"]?.lastSyncedHash).toBe(mergedHash);
    // Verify pull was called with OLD commit as since — the merge commit must be in the pull range
    expect(api.pull).toHaveBeenCalledWith("v", "c0");
  });

  it("creates conflict file when local re-edits between push and pull", async () => {
    const pushedContent = "new";
    const pushedHash = await sha256Text(pushedContent);
    const reEditContent = "re-edit";
    const reEditHash = await sha256Text(reEditContent);
    const mergedContent = "<<<< local\nnew\n====\nremote\n>>>> remote";

    const vault = new FakeVault([
      {
        path: "a.md",
        hash: pushedHash,
        size: pushedContent.length,
        kind: "text",
        content: pushedContent
      }
    ]);
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
      state: vi.fn(),
      pull: vi.fn().mockImplementation(async () => {
        // Simulate local re-edit between push and pull
        vault.files = [
          {
            path: "a.md",
            hash: reEditHash,
            size: reEditContent.length,
            kind: "text",
            content: reEditContent
          }
        ];
        return {
          from: "c0",
          to: "c1",
          added: [],
          modified: [
            {
              path: "a.md",
              file_type: "text",
              size: mergedContent.length,
              content_inline: mergedContent
            }
          ],
          deleted: []
        };
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({
        new_commit: "c1",
        files_changed: 1,
        merge_outcomes: [{ path: "a.md", outcome: "merged" }]
      }),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };

    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "Laptop X",
      textExtensions: new Set(["md"]),
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    // The re-edit hash != pushed hash => dirty check triggers .conflict-* file
    const conflict = [...vault.writes.keys()].find(w => w.includes(".conflict-"));
    expect(conflict).toBeDefined();
    expect(vault.writes.get(conflict!)).toBe(reEditContent);
    // Merged content applied to main file
    expect(vault.writes.get("a.md")).toBe(mergedContent);
  });

  it("handles conflict outcome and fires conflict notice on pull", async () => {
    const pushedHash = await sha256Text("new");
    const conflictContent = "<<<< local\nnew\n====\nremote\n>>>> conflict";

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

    const vault = new FakeVault([
      {
        path: "a.md",
        hash: pushedHash,
        size: 3,
        kind: "text",
        content: "new"
      }
    ]);

    const api = {
      state: vi.fn(),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c1",
        added: [],
        modified: [
          {
            path: "a.md",
            file_type: "text",
            size: conflictContent.length,
            content_inline: conflictContent
          }
        ],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({
        new_commit: "c1",
        files_changed: 1,
        merge_outcomes: [{ path: "a.md", outcome: "conflict", conflict_path: "a.conflict-remote.md" }]
      }),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };

    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    // Push step: head stays at c0
    // Pull step: conflict content arrives, local hash == pushed hash => NOT dirty => clean overwrite
    // After pull, head advances to c1
    expect(idx.saved?.lastSyncedCommit).toBe("c1");
  });

  it("preserves 409 fallback: pull-then-push on head_mismatch", async () => {
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
    const vault = new FakeVault([
      {
        path: "a.md",
        hash: "new",
        size: 3,
        kind: "text",
        content: "new"
      }
    ]);
    const api = {
      state: vi.fn(),
      pull: vi
        .fn()
        // Pull #1: inside 409 retry handler — advances head to c1
        .mockResolvedValueOnce({
          from: "c0",
          to: "c1",
          added: [],
          modified: [],
          deleted: []
        })
        // Pull #2: step ③ backflow pull — up to date after push to c2
        .mockResolvedValueOnce({
          from: "c2",
          to: "c2",
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
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    expect(api.push).toHaveBeenCalledTimes(2);
    expect(idx.saved?.lastSyncedCommit).toBe("c2");
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
      pull: vi
        .fn()
        // Pull #1: inside 409 retry handler — advances head to c1
        .mockResolvedValueOnce({
          from: "c0",
          to: "c1",
          added: [],
          modified: [],
          deleted: []
        })
        // Pull #2: step ③ backflow pull — up to date after push to c2
        .mockResolvedValueOnce({
          from: "c2",
          to: "c2",
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
