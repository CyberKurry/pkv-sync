import { beforeEach, describe, expect, it, vi } from "vitest";
import { subscribeVaultEvents, type SubscribeOptions } from "../../src/api/events-client";
import {
  SyncEngine,
  type IndexPersistence,
  type SyncEngineOptions
} from "../../src/sync/engine";
import { sha256Bytes, sha256Text } from "../../src/sync/hash";
import type { LocalFileSnapshot, LocalIndex } from "../../src/sync/types";
import { shouldSyncPath } from "../../src/sync/vault-adapter";
import { notices } from "../mocks/obsidian";
import { en } from "../../src/i18n/en";

vi.mock("../../src/api/events-client", () => ({
  subscribeVaultEvents: vi.fn()
}));

class FakeVault {
  writes = new Map<string, string>();
  deletions: string[] = [];

  constructor(public files: LocalFileSnapshot[]) {}

  async scan(): Promise<LocalFileSnapshot[]> {
    return this.files.filter((file) => shouldSyncPath(file.path));
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
    vi.mocked(subscribeVaultEvents).mockReset();
  });

  it("uses conditional pull without a state preflight when the remote head is unchanged", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const api = {
      state: vi.fn().mockRejectedValue(new Error("state should not be called")),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c0",
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
      vault: vault as any,
      api: api as any,
      index: idx,
      setStatus: vi.fn()
    });

    await engine.syncNow();

    expect(api.state).not.toHaveBeenCalled();
    expect(api.pull).toHaveBeenCalledWith("v", "c0");
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

  it("reuses downloaded non-inline text content for matching and writing", async () => {
    const oldHash = await sha256Text("old");
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
        added: [],
        modified: [
          {
            path: "a.md",
            file_type: "text",
            size: 6,
            content_inline: null
          }
        ],
        deleted: []
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn(),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn().mockResolvedValue("remote")
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

    expect(api.downloadTextFile).toHaveBeenCalledTimes(1);
    expect(vault.writes.get("a.md")).toBe("remote");
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
      setStatus: vi.fn(),
      vaultSettingsReader: getVaultSettings
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

  it("falls back to a full sync for rollback SSE events without iterating changes", async () => {
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const api = {
      state: vi.fn().mockResolvedValue({
        current_head: "c2",
        changed_since: true
      }),
      pull: vi.fn().mockResolvedValue({
        from: "c0",
        to: "c2",
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
      vault: vault as unknown as SyncEngineOptions["vault"],
      api: api as unknown as SyncEngineOptions["api"],
      index: idx,
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_abc",
      token: "tok",
      deviceId: "dev",
      setStatus: vi.fn()
    });

    engine.startEventSubscription();
    const options = vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions;
    options.onEvent({
      kind: "rollback",
      commit: "c2",
      parent: "c1",
      source_device_id: "other",
      at: 1_700_000_000,
      from_commit: "c1",
      to_commit: "c2"
    });
    await vi.waitFor(() => {
      expect(api.pull).toHaveBeenCalledWith("v", "c0");
    });

    expect(idx.saved?.lastSyncedCommit).toBe("c2");
  });

  it("surfaces non-dirty inline apply failures while falling back to pull", async () => {
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const writeError = Object.assign(new Error("ENOSPC: no space left"), {
      code: "ENOSPC"
    });
    vi.spyOn(vault, "writeText").mockRejectedValueOnce(writeError);
    const setStatus = vi.fn();
    const api = {
      state: vi.fn().mockResolvedValue({
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
      push: vi.fn(),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as unknown as SyncEngineOptions["vault"],
      api: api as unknown as SyncEngineOptions["api"],
      index: idx,
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_abc",
      token: "tok",
      deviceId: "dev",
      labels: en,
      setStatus
    });

    engine.startEventSubscription();
    const options = vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions;
    options.onEvent({
      kind: "commit",
      commit: "c1",
      parent: "c0",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [
        {
          kind: "text_inline",
          path: "space.md",
          content: "remote"
        }
      ]
    });

    await vi.waitFor(() => {
      expect(api.pull).toHaveBeenCalledWith("v", "c0");
    });

    const message =
      "Failed to apply realtime update for space.md: ENOSPC: no space left. Falling back to full sync.";
    expect(notices).toContain(message);
    expect(setStatus).toHaveBeenCalledWith("error", message);
  });

  it("silently falls back to pull when inline apply finds dirty local content", async () => {
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());
    const cleanHash = await sha256Text("clean");
    const dirtyHash = await sha256Text("local edit");
    const idx = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "dirty.md": {
          lastSyncedHash: cleanHash,
          lastSyncedAt: 1,
          kind: "text",
          size: 5
        }
      }
    });
    const vault = new FakeVault([
      {
        path: "dirty.md",
        hash: dirtyHash,
        size: 10,
        kind: "text",
        content: "local edit"
      }
    ]);
    const setStatus = vi.fn();
    const api = {
      state: vi.fn().mockResolvedValue({
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
      push: vi.fn().mockResolvedValue({ new_commit: "c2", files_changed: 1 }),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as unknown as SyncEngineOptions["vault"],
      api: api as unknown as SyncEngineOptions["api"],
      index: idx,
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_abc",
      token: "tok",
      deviceId: "dev",
      labels: en,
      setStatus
    });

    engine.startEventSubscription();
    const options = vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions;
    options.onEvent({
      kind: "commit",
      commit: "c1",
      parent: "c0",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [
        {
          kind: "text_inline",
          path: "dirty.md",
          content: "remote"
        }
      ]
    });

    await vi.waitFor(() => {
      expect(api.pull).toHaveBeenCalledWith("v", "c0");
    });

    expect(notices).toEqual([]);
    expect(setStatus).not.toHaveBeenCalledWith("error", expect.any(String));
  });

  it("continues processing later SSE events after a fallback sync failure", async () => {
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const originalWriteText = vault.writeText.bind(vault);
    vi.spyOn(vault, "writeText")
      .mockRejectedValueOnce(new Error("disk full"))
      .mockImplementation(originalWriteText);
    const setStatus = vi.fn();
    const api = {
      state: vi.fn(),
      pull: vi
        .fn()
        .mockRejectedValueOnce(new Error("server offline"))
        .mockResolvedValue({
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
      vault: vault as unknown as SyncEngineOptions["vault"],
      api: api as unknown as SyncEngineOptions["api"],
      index: idx,
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_abc",
      token: "tok",
      deviceId: "dev",
      labels: en,
      setStatus
    });

    engine.startEventSubscription();
    const options = vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions;
    options.onEvent({
      kind: "commit",
      commit: "c1",
      parent: "c0",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [
        {
          kind: "text_inline",
          path: "first.md",
          content: "first"
        }
      ]
    });
    await vi.waitFor(() => {
      expect(api.pull).toHaveBeenCalledTimes(1);
    });

    options.onEvent({
      kind: "commit",
      commit: "c2",
      parent: "c1",
      source_device_id: "other",
      at: 1_700_000_001,
      changes: [
        {
          kind: "text_inline",
          path: "second.md",
          content: "second"
        }
      ]
    });

    await vi.waitFor(() => {
      expect(vault.writes.get("second.md")).toBe("second");
    });
    expect(idx.saved?.lastSyncedCommit).toBe("c2");
  });
});
