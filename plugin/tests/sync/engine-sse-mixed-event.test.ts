import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  subscribeVaultEvents,
  type SubscribeOptions
} from "../../src/api/events-client";
import {
  SyncEngine,
  type IndexPersistence,
  type SyncEngineOptions
} from "../../src/sync/engine";
import { sha256Bytes, sha256Text } from "../../src/sync/hash";
import type {
  LocalFileSnapshot,
  LocalIndex,
  PullResponse
} from "../../src/sync/types";
import { shouldSyncPath } from "../../src/sync/vault-adapter";

vi.mock("../../src/api/events-client", () => ({
  subscribeVaultEvents: vi.fn(),
  SseAuthError: class SseAuthError extends Error {}
}));

class FakeVault {
  configDir = ".obsidian";
  writes = new Map<string, string>();
  binaryWrites = new Map<string, Uint8Array>();
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
      size: content.length,
      kind: "text",
      content
    };
    this.files = this.files.filter((file) => file.path !== path).concat(next);
  }
  async writeBinary(path: string, data: ArrayBuffer): Promise<void> {
    const bytes = new Uint8Array(data);
    this.binaryWrites.set(path, bytes);
    const next: LocalFileSnapshot = {
      path,
      hash: await sha256Bytes(data),
      size: bytes.byteLength,
      kind: "blob",
      bytes: data
    };
    this.files = this.files.filter((file) => file.path !== path).concat(next);
  }
  async trash(path: string): Promise<void> {
    this.deletions.push(path);
    this.files = this.files.filter((file) => file.path !== path);
  }

  /** The real adapter re-reads the file; the R15-5 recheck relies on it. */
  async snapshot(
    path: string,
    _textExtensions?: Set<string>
  ): Promise<LocalFileSnapshot> {
    const file = this.files.find((entry) => entry.path === path);
    if (!file) throw new Error(`not found: ${path}`);
    return file;
  }
}

class FakeIndex implements IndexPersistence {
  saved: LocalIndex | null = null;
  constructor(public idx: LocalIndex) {}
  async loadIndex(): Promise<LocalIndex> {
    return this.idx;
  }
  async updateIndex(
    updater: (index: LocalIndex) => LocalIndex | Promise<LocalIndex>
  ): Promise<void> {
    const next = await updater(this.idx);
    this.saved = next;
    this.idx = next;
  }
}

describe("SSE mixed events must not strand sibling changes (R14-H1)", () => {
  beforeEach(() => {
    vi.mocked(subscribeVaultEvents).mockReset();
    vi.stubGlobal("window", globalThis);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("blob sibling of an applied text_inline is delivered by the fallback pull", async () => {
    // One server push batch → ONE SSE event. The batch carries a text note
    // (inline) plus a binary attachment (EventChange::Blob — blobs are never
    // inlined). The server's pull is a tree-state diff between `since` and
    // head: since === head returns EMPTY — commits already at the index
    // position are never re-delivered. So if the inline apply advanced the
    // index head to the event commit BEFORE the fallback pull ran, the blob
    // sibling was stranded permanently (R14-H1).
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());

    const pngBytes = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 1, 2, 3, 4]);
    const pngHash = await sha256Bytes(pngBytes.buffer as ArrayBuffer);

    const vault = new FakeVault([]);
    const index = new FakeIndex({ lastSyncedCommit: "c0", files: {} });

    const pull = vi.fn(async (_vaultId: string, since: string | null): Promise<PullResponse> => {
      if (since === "c0" || since === null) {
        return {
          from: "c0",
          to: "c1",
          added: [
            { path: "a.md", file_type: "text", size: 5, content_inline: "alpha" },
            {
              path: "b.png",
              file_type: "blob",
              size: pngBytes.byteLength,
              blob_hash: pngHash
            }
          ],
          modified: [],
          deleted: []
        };
      }
      // Real server semantics: head == since → nothing to deliver.
      return { from: since, to: since, added: [], modified: [], deleted: [] };
    });
    const api = {
      state: vi.fn(async () => ({ current_head: "c1", changed_since: true })),
      pull,
      push: vi.fn(),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      downloadBlob: vi.fn(async () => pngBytes.buffer as ArrayBuffer),
      downloadTextFile: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceId: "dev-self",
      deviceName: "d",
      serverUrl: "http://test",
      deploymentKey: "k",
      token: "t",
      textExtensions: new Set(["md"]),
      vault: vault as unknown as SyncEngineOptions["vault"],
      api: api as unknown as SyncEngineOptions["api"],
      index,
      setStatus: vi.fn()
    });
    engine.startEventSubscription();
    const options = vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions;

    void options.onEvent({
      kind: "commit",
      commit: "c1",
      parent: "c0",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [
        { kind: "text_inline", path: "a.md", content: "alpha" },
        { kind: "blob", path: "b.png", blob_hash: pngHash, size: pngBytes.byteLength }
      ]
    });
    await vi.waitFor(() => {
      // The inline text must have been applied...
      expect(vault.writes.get("a.md")).toBe("alpha");
      // ...and the fallback pull must have run.
      expect(pull).toHaveBeenCalled();
    });
    // Let the fallback pass settle, then demand convergence: the blob
    // sibling must reach the vault, not be stranded behind an index head
    // the tree-diff can never cross.
    await vi.waitFor(() => {
      expect(vault.binaryWrites.get("b.png")).toBeDefined();
      expect(index.idx.lastSyncedCommit).toBe("c1");
    });
    // The fallback pull covered the event's whole range: it pulled from
    // the parent, not from the already-applied commit.
    const sinceArgs = pull.mock.calls.map((call) => call[1]);
    expect(sinceArgs).toContain("c0");
  });

  it("failed inline sibling keeps the head at the parent so the pull covers the event", async () => {
    // Same hazard with text_ref: the inline apply of the FIRST change must
    // not advance the head past a SECOND change the event still holds as a
    // ref (needs a fetch the pull path performs).
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());

    const vault = new FakeVault([]);
    const index = new FakeIndex({ lastSyncedCommit: "c0", files: {} });

    const pull = vi.fn(async (_vaultId: string, since: string | null): Promise<PullResponse> => {
      if (since === "c0" || since === null) {
        return {
          from: "c0",
          to: "c1",
          added: [
            { path: "a.md", file_type: "text", size: 5, content_inline: "alpha" },
            { path: "b.md", file_type: "text", size: 4, content_inline: "beta" }
          ],
          modified: [],
          deleted: []
        };
      }
      return { from: since, to: since, added: [], modified: [], deleted: [] };
    });
    const api = {
      state: vi.fn(async () => ({ current_head: "c1", changed_since: true })),
      pull,
      push: vi.fn(),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceId: "dev-self",
      deviceName: "d",
      serverUrl: "http://test",
      deploymentKey: "k",
      token: "t",
      textExtensions: new Set(["md"]),
      vault: vault as unknown as SyncEngineOptions["vault"],
      api: api as unknown as SyncEngineOptions["api"],
      index,
      setStatus: vi.fn()
    });
    engine.startEventSubscription();
    const options = vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions;

    void options.onEvent({
      kind: "commit",
      commit: "c1",
      parent: "c0",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [
        { kind: "text_inline", path: "a.md", content: "alpha" },
        { kind: "text_ref", path: "b.md", size: 4 }
      ]
    });
    await vi.waitFor(() => {
      expect(vault.writes.get("a.md")).toBe("alpha");
      expect(pull).toHaveBeenCalled();
    });
    await vi.waitFor(() => {
      expect(vault.writes.get("b.md")).toBe("beta");
      expect(index.idx.lastSyncedCommit).toBe("c1");
    });
    expect(pull.mock.calls.map((call) => call[1])).toContain("c0");
  });
});

describe("SSE apply honours the local exclusion rules (R15-6)", () => {
  beforeEach(() => {
    vi.mocked(subscribeVaultEvents).mockReset();
    vi.stubGlobal("window", globalThis);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  function engineFor(opts: {
    files: LocalFileSnapshot[];
    index: LocalIndex;
    extraExcludeGlobs: string[];
  }) {
    const vault = new FakeVault(opts.files);
    const index = new FakeIndex(opts.index);
    const engine = new SyncEngine({
      vaultId: "v",
      deviceId: "dev-self",
      deviceName: "d",
      serverUrl: "http://test",
      deploymentKey: "k",
      token: "t",
      textExtensions: new Set(["md"]),
      extraExcludeGlobs: opts.extraExcludeGlobs,
      vault: vault as unknown as SyncEngineOptions["vault"],
      api: {
        state: vi.fn(async () => ({ current_head: "c1", changed_since: true })),
        pull: vi.fn(async (): Promise<PullResponse> => ({
          from: "c1",
          to: "c1",
          added: [],
          modified: [],
          deleted: []
        })),
        push: vi.fn(),
        uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
        uploadBlob: vi.fn(),
        downloadBlob: vi.fn(),
        downloadTextFile: vi.fn()
      } as unknown as SyncEngineOptions["api"],
      index,
      setStatus: vi.fn()
    });
    engine.startEventSubscription();
    return { engine, vault, index, options: vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions };
  }

  it("does not trash a locally excluded path on a remote delete", async () => {
    // The pull path filters remote changes through shouldSyncPath and the
    // path matcher; the SSE loop called applyInlineText / applyDelete
    // directly. A vault whose local excludes are NARROWER than the
    // server's (e.g. `.obsidian/plugins/**` kept out of sync on purpose)
    // therefore had those files trashed by a remote delete — a local
    // exclusion silently overridden by the realtime stream.
    const { vault, options } = engineFor({
      files: [
        {
          path: ".obsidian/plugins/x/data.json",
          hash: "h1",
          size: 2,
          kind: "text",
          content: "{}"
        }
      ],
      index: {
        lastSyncedCommit: "c0",
        files: {
          ".obsidian/plugins/x/data.json": {
            lastSyncedHash: "h1",
            lastSyncedAt: 1,
            kind: "text",
            size: 2
          }
        }
      },
      extraExcludeGlobs: [".obsidian/plugins/**"]
    });

    void options.onEvent({
      kind: "commit",
      commit: "c1",
      parent: "c0",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [{ kind: "delete", path: ".obsidian/plugins/x/data.json" }]
    });

    // Wait for the handler to finish, then assert. A negative assertion
    // under waitFor would pass instantly, before the handler ran at all —
    // which is exactly the false green this test had on its first run.
    await vi.waitFor(() => expect(options).toBeDefined());
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(vault.deletions).not.toContain(".obsidian/plugins/x/data.json");
    expect(vault.exists(".obsidian/plugins/x/data.json")).toBe(true);
  });

  it("does not write a locally excluded path on a remote inline update", async () => {
    const { vault, options } = engineFor({
      files: [
        {
          path: ".obsidian/plugins/x/data.json",
          hash: "h1",
          size: 2,
          kind: "text",
          content: "{}"
        }
      ],
      // (the local file exists and is indexed clean, so without a path
      // filter the remote body would overwrite it)
      index: {
        lastSyncedCommit: "c0",
        files: {
          ".obsidian/plugins/x/data.json": {
            lastSyncedHash: "h1",
            lastSyncedAt: 1,
            kind: "text",
            size: 2
          }
        }
      },
      extraExcludeGlobs: [".obsidian/plugins/**"]
    });

    void options.onEvent({
      kind: "commit",
      commit: "c1",
      parent: "c0",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [
        {
          kind: "text_inline",
          path: ".obsidian/plugins/x/data.json",
          content: "{\"remote\":true}"
        }
      ]
    });

    await vi.waitFor(() => expect(options).toBeDefined());
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(vault.writes.size).toBe(0);
    expect(vault.exists(".obsidian/plugins/x/data.json")).toBe(true);
  });
});
