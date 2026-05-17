import { describe, expect, it, vi } from "vitest";
import {
  InlineApplyDirtyError,
  SyncEngine,
  type IndexPersistence
} from "../../src/sync/engine";
import { sha256Text } from "../../src/sync/hash";
import type { LocalIndex } from "../../src/sync/types";
import { shouldSyncPath } from "../../src/sync/vault-adapter";

class FakeVault {
  writes = new Map<string, string>();
  deletions: string[] = [];
  store = new Map<string, string>();

  async scan() {
    return Array.from(this.store.entries())
      .filter(([p]) => shouldSyncPath(p))
      .map(async ([path, content]) => ({
        path,
        hash: await sha256Text(content),
        size: new TextEncoder().encode(content).byteLength,
        kind: "text" as const,
        content
      }));
  }

  exists(path: string): boolean {
    return this.store.has(path);
  }

  async readText(path: string): Promise<string> {
    const value = this.store.get(path);
    if (value === undefined) throw new Error(`not found: ${path}`);
    return value;
  }

  async writeText(path: string, content: string): Promise<void> {
    this.writes.set(path, content);
    this.store.set(path, content);
  }

  async writeBinary(): Promise<void> {
    throw new Error("not implemented for these tests");
  }

  async delete(path: string): Promise<void> {
    this.deletions.push(path);
    this.store.delete(path);
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
}

function buildEngine(opts: {
  vault: FakeVault;
  index: FakeIndex;
}): SyncEngine {
  const api = {
    state: vi.fn(),
    pull: vi.fn(),
    push: vi.fn(),
    uploadCheck: vi.fn(),
    uploadBlob: vi.fn(),
    downloadBlob: vi.fn(),
    readFile: vi.fn()
  };
  return new SyncEngine({
    vaultId: "v1",
    deviceId: "dev-self",
    deviceName: "test-device",
    serverUrl: "http://test",
    deploymentKey: "k",
    token: "t",
    api: api as never,
    vault: opts.vault as never,
    index: opts.index,
    textExtensions: new Set(["md"]),
    extraExcludeGlobs: [],
    setStatus: () => undefined
  });
}

describe("SyncEngine inline apply dirty detection", () => {
  it("writes content when no local file exists (new file from remote)", async () => {
    const vault = new FakeVault();
    const cleanHash = await sha256Text("");
    const index = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {}
    });
    const engine = buildEngine({ vault, index });

    await (engine as never as { applyInlineText: (p: string, c: string, sha: string) => Promise<void> })
      .applyInlineText("note.md", "remote content", "c1");

    expect(vault.writes.get("note.md")).toBe("remote content");
    expect(index.saved?.lastSyncedCommit).toBe("c1");
    expect(index.saved?.files["note.md"]?.lastSyncedHash).toBe(await sha256Text("remote content"));
    expect(cleanHash).toBeTypeOf("string");
  });

  it("writes content when local matches index (clean state)", async () => {
    const vault = new FakeVault();
    vault.store.set("note.md", "synced text");
    const cleanHash = await sha256Text("synced text");
    const index = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "note.md": {
          lastSyncedHash: cleanHash,
          lastSyncedAt: 1,
          size: 11,
          kind: "text"
        }
      }
    });
    const engine = buildEngine({ vault, index });

    await (engine as never as { applyInlineText: (p: string, c: string, sha: string) => Promise<void> })
      .applyInlineText("note.md", "new remote content", "c1");

    expect(vault.writes.get("note.md")).toBe("new remote content");
    expect(index.saved?.files["note.md"]?.lastSyncedHash).toBe(
      await sha256Text("new remote content")
    );
  });

  it("throws InlineApplyDirtyError when local has unsynced changes", async () => {
    const vault = new FakeVault();
    vault.store.set("note.md", "user-edited-locally");
    const indexedHash = await sha256Text("original synced content");
    const index = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "note.md": {
          lastSyncedHash: indexedHash,
          lastSyncedAt: 1,
          size: 23,
          kind: "text"
        }
      }
    });
    const engine = buildEngine({ vault, index });

    await expect(
      (engine as never as { applyInlineText: (p: string, c: string, sha: string) => Promise<void> })
        .applyInlineText("note.md", "remote replacement", "c1")
    ).rejects.toBeInstanceOf(InlineApplyDirtyError);

    // Crucially, local file was NOT overwritten
    expect(vault.store.get("note.md")).toBe("user-edited-locally");
    expect(vault.writes.size).toBe(0);
    // And index was NOT advanced
    expect(index.saved).toBeNull();
  });
});
