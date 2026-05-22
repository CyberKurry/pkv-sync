import { beforeEach, describe, expect, it, vi } from "vitest";
import { SyncEngine, type IndexPersistence } from "../../src/sync/engine";
import { sha256Bytes, sha256Text } from "../../src/sync/hash";
import type { LocalFileSnapshot, LocalIndex } from "../../src/sync/types";
import { shouldSyncPath } from "../../src/sync/vault-adapter";
import { notices } from "../mocks/obsidian";

class FakeVault {
  writes = new Map<string, string>();
  deletions: string[] = [];

  constructor(public files: LocalFileSnapshot[]) {}

  async scan(): Promise<LocalFileSnapshot[]> {
    return this.files.filter((file) => shouldSyncPath(file.path));
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
  saves: LocalIndex[] = [];
  saved: LocalIndex | null = null;

  constructor(public idx: LocalIndex) {}

  async loadIndex(): Promise<LocalIndex> {
    return this.idx;
  }

  async saveIndex(index: LocalIndex): Promise<void> {
    this.saves.push(index);
    this.saved = index;
    this.idx = index;
  }

  async updateIndex(
    updater: (index: LocalIndex) => LocalIndex | Promise<LocalIndex>
  ): Promise<void> {
    const next = await updater(this.idx);
    this.saves.push(next);
    this.saved = next;
    this.idx = next;
  }
}

describe("SyncEngine pull", () => {
  beforeEach(() => {
    notices.length = 0;
  });

  it("applies inline text pull and updates index without re-pushing it", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: "c1",
        changed_since: true
      }),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c1",
        added: [
          {
            path: "a.md",
            file_type: "text",
            size: 2,
            content_inline: "hi"
          }
        ],
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
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    expect(vault.writes.get("a.md")).toBe("hi");
    expect(idx.saved?.lastSyncedCommit).toBe("c1");
    expect(api.push).not.toHaveBeenCalled();
  });

  it("downloads non-inline text content before writing", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: "c1",
        changed_since: true
      }),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c1",
        added: [
          {
            path: "large.md",
            file_type: "text",
            size: 70000,
            content_inline: null
          }
        ],
        modified: [],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn(),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn().mockResolvedValue("large content")
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

    expect(api.downloadTextFile).toHaveBeenCalledWith("v", "large.md", "c1");
    expect(vault.writes.get("large.md")).toBe("large content");
  });

  it("preserves dirty local text as a conflict file before applying remote", async () => {
    const cleanHash = await sha256Text("clean");
    const dirtyHash = await sha256Text("local");
    const idx = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "a.md": {
          lastSyncedHash: cleanHash,
          lastSyncedAt: 1,
          kind: "text",
          size: 5
        }
      }
    });
    const vault = new FakeVault([
      {
        path: "a.md",
        hash: dirtyHash,
        size: 5,
        kind: "text",
        content: "local"
      }
    ]);
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: "c1",
        changed_since: true
      }),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c1",
        added: [],
        modified: [
          {
            path: "a.md",
            file_type: "text",
            size: 6,
            content_inline: "remote"
          }
        ],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({ new_commit: "c2", files_changed: 1 }),
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

    const conflict = [...vault.writes.keys()].find((path) =>
      path.includes(".conflict-")
    );
    expect(conflict).toMatch(/^a\.conflict-\d{4}-\d{2}-\d{2}-\d{6}-Laptop-X\.md$/);
    expect(vault.writes.get(conflict!)).toBe("local");
    expect(vault.writes.get("a.md")).toBe("remote");
    expect(notices[0]).toContain("PKV Sync conflict");
  });

  it("adopts matching local text without creating conflicts on first pull", async () => {
    const content = "same notes";
    const hash = await sha256Text(content);
    const idx = new FakeIndex({ lastSyncedCommit: null, files: {} });
    const vault = new FakeVault([
      {
        path: "a.md",
        hash,
        size: new TextEncoder().encode(content).byteLength,
        kind: "text",
        content
      }
    ]);
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: "c1",
        changed_since: true
      }),
      pull: vi.fn().mockResolvedValue({
        from: null,
        to: "c1",
        added: [
          {
            path: "a.md",
            file_type: "text",
            size: new TextEncoder().encode(content).byteLength,
            content_inline: content
          }
        ],
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
      deviceName: "Laptop X",
      textExtensions: new Set(["md"]),
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    expect([...vault.writes.keys()].filter((path) => path.includes(".conflict-"))).toEqual([]);
    expect(vault.writes.get("a.md")).toBeUndefined();
    expect(idx.saved?.lastSyncedCommit).toBe("c1");
    expect(idx.saved?.files["a.md"]?.lastSyncedHash).toBe(hash);
    expect(api.push).not.toHaveBeenCalled();
    expect(notices).toEqual([]);
  });

  it("skips forbidden remote paths while advancing the pull checkpoint", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([
      {
        path: ".trash/deleted.md",
        hash: await sha256Text("local trash"),
        size: 11,
        kind: "text",
        content: "local trash"
      }
    ]);
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: "c1",
        changed_since: true
      }),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c1",
        added: [
          {
            path: ".obsidian/workspace.json",
            file_type: "text",
            size: 2,
            content_inline: "{}"
          }
        ],
        modified: [],
        deleted: [".trash/deleted.md"]
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
      textExtensions: new Set(["md", "json"]),
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    expect(vault.writes.has(".obsidian/workspace.json")).toBe(false);
    expect(vault.deletions).not.toContain(".trash/deleted.md");
    expect(idx.saved?.lastSyncedCommit).toBe("c1");
    expect(api.push).not.toHaveBeenCalled();
  });

  it("applies server-generated conflict files from full pull", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const conflictPath = "note.conflict-2026-05-23-001122-server.md";
    const marked = [
      "<<<<<<< local",
      "local",
      "=======",
      "remote",
      ">>>>>>> remote",
      ""
    ].join("\n");
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: "c1",
        changed_since: true
      }),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c1",
        added: [
          {
            path: conflictPath,
            file_type: "text",
            size: new TextEncoder().encode(marked).byteLength,
            content_inline: null
          }
        ],
        modified: [],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn(),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn().mockResolvedValue(marked)
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

    expect(api.downloadTextFile).toHaveBeenCalledWith("v", conflictPath, "c1");
    expect(vault.writes.get(conflictPath)).toBe(marked);
    expect(idx.saved?.files[conflictPath]).toBeUndefined();
    expect(idx.saved?.lastSyncedCommit).toBe("c1");
    expect(api.push).not.toHaveBeenCalled();
  });

  it("applies allowlisted .obsidian pull files and skips non-allowlisted plugin code", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const getVaultSettings = vi.fn().mockResolvedValue({
      extra_sync_globs: [".obsidian/themes/**"]
    });
    const api = {
      api: { getVaultSettings },
      state: vi.fn().mockResolvedValue({
        current_head: "c1",
        changed_since: true
      }),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c1",
        added: [
          {
            path: ".obsidian/themes/cyberkurry-dark/manifest.json",
            file_type: "text",
            size: 16,
            content_inline: "{\"name\":\"dark\"}"
          },
          {
            path: ".obsidian/plugins/example/main.js",
            file_type: "text",
            size: 8,
            content_inline: "plugin()"
          }
        ],
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
      textExtensions: new Set(["md", "json", "js"]),
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    expect(vault.writes.get(".obsidian/themes/cyberkurry-dark/manifest.json")).toBe(
      "{\"name\":\"dark\"}"
    );
    expect(vault.writes.has(".obsidian/plugins/example/main.js")).toBe(false);
    expect(idx.saved?.files[".obsidian/themes/cyberkurry-dark/manifest.json"]).toBeDefined();
    expect(idx.saved?.files[".obsidian/plugins/example/main.js"]).toBeUndefined();
    expect(api.push).not.toHaveBeenCalled();
  });

  it("keeps local deletion intent when remote modifies the same file", async () => {
    const cleanHash = await sha256Text("clean");
    const idx = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "a.md": {
          lastSyncedHash: cleanHash,
          lastSyncedAt: 1,
          kind: "text",
          size: 5
        }
      }
    });
    const vault = new FakeVault([]);
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: "c1",
        changed_since: true
      }),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c1",
        added: [],
        modified: [
          {
            path: "a.md",
            file_type: "text",
            size: 6,
            content_inline: "remote"
          }
        ],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({ new_commit: "c2", files_changed: 1 }),
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

    const conflict = [...vault.writes.keys()].find((path) =>
      path.includes(".conflict-")
    );
    expect(vault.writes.has("a.md")).toBe(false);
    expect(conflict).toMatch(/^a\.conflict-\d{4}-\d{2}-\d{2}-\d{6}-remote\.md$/);
    expect(vault.writes.get(conflict!)).toBe("remote");
    expect(api.push).toHaveBeenCalledWith("v", "c1", [
      { kind: "delete", path: "a.md" }
    ], "Laptop X");
  });

  it("records files applied before a pull failure without advancing the commit", async () => {
    const oldHash = await sha256Text("old");
    const remoteHash = await sha256Text("remote");
    const idx = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "a.md": {
          lastSyncedHash: oldHash,
          lastSyncedAt: 1,
          kind: "text",
          size: 3
        }
      }
    });
    const vault = new FakeVault([
      {
        path: "a.md",
        hash: oldHash,
        size: 3,
        kind: "text",
        content: "old"
      }
    ]);
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: "c1",
        changed_since: true
      }),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c1",
        added: [
          {
            path: "a.md",
            file_type: "text",
            size: 6,
            content_inline: "remote"
          },
          {
            path: "b.md",
            file_type: "text",
            size: 6,
            content_inline: "fail"
          }
        ],
        modified: [],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({ new_commit: "c2", files_changed: 1 }),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };
    const originalWriteText = vault.writeText.bind(vault);
    vi.spyOn(vault, "writeText").mockImplementation(async (path, content) => {
      if (path === "b.md") throw new Error("disk full");
      await originalWriteText(path, content);
    });
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await expect(engine.syncNow()).rejects.toThrow("disk full");

    expect(idx.saves).toHaveLength(1);
    expect(idx.saved?.lastSyncedCommit).toBe("c0");
    expect(idx.saved?.files["a.md"]?.lastSyncedHash).toBe(remoteHash);
    expect(idx.saved?.files["b.md"]).toBeUndefined();
  });

  it("rejects downloaded blobs whose bytes do not match the advertised hash", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const advertisedHash = await sha256Bytes(
      new TextEncoder().encode("expected").buffer
    );
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: "c1",
        changed_since: true
      }),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c1",
        added: [
          {
            path: "image.png",
            file_type: "blob",
            size: 7,
            blob_hash: advertisedHash
          }
        ],
        modified: [],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn().mockResolvedValue({ new_commit: "c2", files_changed: 1 }),
      downloadBlob: vi
        .fn()
        .mockResolvedValue(new TextEncoder().encode("corrupt").buffer),
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

    await expect(engine.syncNow()).rejects.toThrow("Blob hash mismatch");

    expect(vault.files).toHaveLength(0);
    expect(idx.saves).toHaveLength(0);
  });
});
