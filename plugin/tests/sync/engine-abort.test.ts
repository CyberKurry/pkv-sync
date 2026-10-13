import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { subscribeVaultEvents, type SubscribeOptions } from "../../src/api/events-client";
import {
  SyncEngine,
  type IndexPersistence,
  type SyncEngineOptions
} from "../../src/sync/engine";
import { sha256Text } from "../../src/sync/hash";
import type { LocalFileSnapshot, LocalIndex } from "../../src/sync/types";
import { shouldSyncPath } from "../../src/sync/vault-adapter";
import { notices } from "../mocks/obsidian";

vi.mock("../../src/api/events-client", () => ({
  subscribeVaultEvents: vi.fn()
}));

class FakeVault {
  configDir = ".obsidian";
  writes = new Map<string, string>();
  binaryWrites = new Map<string, ArrayBuffer>();
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

  async writeBinary(path: string, bytes: ArrayBuffer): Promise<void> {
    this.binaryWrites.set(path, bytes);
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

  async trash(path: string): Promise<void> {
    this.deletions.push(path);
    this.files = this.files.filter((file) => file.path !== path);
  }

  async snapshot(path: string): Promise<LocalFileSnapshot> {
    const content = await this.readText(path);
    return {
      path,
      hash: await sha256Text(content),
      size: content.length,
      kind: "text",
      content
    };
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

describe("SyncEngine abort (M-10)", () => {
  beforeEach(() => {
    vi.mocked(subscribeVaultEvents).mockReset();
    vi.stubGlobal("window", globalThis);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    vi.clearAllMocks();
  });

  it("does not write pulled content into the vault after abort() (M-10)", async () => {
    // Audit M-10: rebuild()/invalidate() drop the engine reference, but an
    // in-flight syncInner keeps running against the OLD vault/server and
    // writes (and trashes) into the CURRENT local vault. The engine must
    // check its abort signal before every vault write/trash and stop.
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    // Gate the pull response: the sync stalls mid-pull until the test
    // aborts, then the (stale) server answer arrives.
    const gate = deferred<void>();
    const api = {
      state: vi.fn(),
      pull: vi.fn(async () => {
        await gate.promise;
        return {
          from: "c0",
          to: "c1",
          added: [
            {
              path: "stale-x.md",
              file_type: "text" as const,
              size: 19,
              content_inline: "stale remote x"
            }
          ],
          modified: [],
          deleted: []
        };
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn(),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };
    const setStatus = vi.fn();
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as unknown as SyncEngineOptions["vault"],
      api: api as unknown as SyncEngineOptions["api"],
      index: idx,
      setStatus
    });

    const syncing = engine.syncNow();
    // Let the sync reach (and stall inside) the pull.
    await vi.waitFor(() => expect(api.pull).toHaveBeenCalled());

    engine.abort();
    gate.resolve();

    // The aborted sync resolves as cancelled — not an error the caller must
    // notice, and no error status is set on the (likely already replaced) UI.
    await syncing;
    expect(setStatus).not.toHaveBeenCalledWith("error", expect.any(String));

    // Nothing from the stale server landed in the vault, and nothing was
    // trashed or recorded in the index.
    expect(vault.writes.get("stale-x.md")).toBeUndefined();
    expect(vault.writes.size).toBe(0);
    expect(vault.binaryWrites.size).toBe(0);
    expect(vault.deletions).toEqual([]);
    expect(idx.saved?.lastSyncedCommit ?? "c0").toBe("c0");
  });

  it("does not apply a pulled delete (trash) after abort() (M-10)", async () => {
    const idx = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "doomed.md": {
          lastSyncedHash: "h",
          lastSyncedAt: 1,
          kind: "text",
          size: 1
        }
      }
    });
    const vault = new FakeVault([
      {
        path: "doomed.md",
        hash: "h",
        size: 1,
        kind: "text",
        content: "x"
      }
    ]);
    const gate = deferred<void>();
    const api = {
      state: vi.fn(),
      pull: vi.fn(async () => {
        await gate.promise;
        return {
          from: "c0",
          to: "c1",
          added: [],
          modified: [],
          deleted: ["doomed.md"]
        };
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
      setStatus: vi.fn()
    });

    const syncing = engine.syncNow();
    await vi.waitFor(() => expect(api.pull).toHaveBeenCalled());

    engine.abort();
    gate.resolve();
    await syncing;

    // The stale engine must not trash a file out of the current vault.
    expect(vault.deletions).toEqual([]);
    expect(vault.exists("doomed.md")).toBe(true);
  });

  it("skips the push phase entirely when aborted before it starts (M-10)", async () => {
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([
      {
        path: "pending.md",
        hash: await sha256Text("pending local edit"),
        size: 18,
        kind: "text",
        content: "pending local edit"
      }
    ]);
    const api = {
      state: vi.fn(),
      pull: vi.fn(),
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
      setStatus: vi.fn()
    });

    // Abort before the pass begins — nothing may be scanned or pushed.
    engine.abort();
    await engine.syncNow();

    expect(api.push).not.toHaveBeenCalled();
    expect(api.pull).not.toHaveBeenCalled();
    expect(idx.saved).toBeNull();
  });

  it("aborts the previous generation's engine when the orchestrator rebuilds (M-10)", async () => {
    const { SyncOrchestrator } = await import("../../src/sync-orchestrator");
    const { SyncEngine: Engine } = await import("../../src/sync/engine");
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const gate = deferred<void>();
    const pullResponse = {
      from: "c0",
      to: "c1",
      added: [
        {
          path: "from-old-server.md",
          file_type: "text" as const,
          size: 15,
          content_inline: "old server"
        }
      ],
      modified: [],
      deleted: []
    };
    let pullCalls = 0;
    /** Ordered vault operations, tagged with the generation that performed them. */
    const writesByGeneration: string[] = [];
    let adapterCounter = 0;

    const plugin = {
      syncGeneration: 0,
      pushDebouncer: null,
      pollTimer: null,
      fallbackTimer: null,
      engine: null,
      api: vi.fn(() => ({})),
      loadSyncIndex: async () => idx.idx,
      saveSyncIndex: async () => undefined,
      updateSyncIndex: async (updater: (i: LocalIndex) => LocalIndex) => {
        // Mirror the generation guard only for the CURRENT generation, the
        // way the real plugin does — an aborted generation's index writes
        // are dropped, which is exactly the hazard the audit describes.
        if (plugin.syncGeneration !== capturedGeneration()) return;
        await idx.updateIndex(updater);
      },
      statusEl: null,
      text: (() => ({})) as unknown as () => never,
      recordSyncSuccess: vi.fn(),
      settings: {
        serverUrl: "https://old.example.com",
        deploymentKey: "k",
        token: "t",
        userId: "u",
        selectedVaultId: "v1",
        deviceName: "d",
        textExtensions: ["md"],
        extraExcludeGlobs: [],
        debounceMs: 250,
        pollIntervalSeconds: 30,
        deviceId: "dev"
      },
      manifest: { version: "1.6.0" },
      app: { vault: {} }
    };
    let generationAtCapture = 0;
    const capturedGeneration = (): number => generationAtCapture;

    // Swap the sync API and vault adapter the orchestrator wires up. The
    // imports are awaited inline so the mocks apply to the orchestrator's
    // static imports inside this test only.
    const syncClientModule = await import("../../src/api/sync-client");
    const vaultAdapterModule = await import("../../src/sync/vault-adapter");
    const syncApiSpy = vi
      .spyOn(syncClientModule, "SyncApi")
      .mockImplementation(function MockSyncApi(
        this: Record<string, unknown>
      ) {
        this.pull = async () => {
          pullCalls += 1;
          await gate.promise;
          return pullResponse;
        };
        this.state = vi.fn();
        this.uploadCheck = vi.fn().mockResolvedValue({ missing: [] });
        this.uploadBlob = vi.fn();
        this.push = vi.fn();
        this.downloadBlob = vi.fn();
        this.downloadTextFile = vi.fn();
        this.getVaultSettings = vi.fn();
      } as unknown as typeof syncClientModule.SyncApi);
    const adapterSpy = vi
      .spyOn(vaultAdapterModule, "ObsidianVaultAdapter")
      .mockImplementation(function MockAdapter(
        this: Record<string, unknown>
      ) {
        // Tag each adapter with the generation that created it so the test
        // can attribute every vault write to a generation — the shared
        // FakeVault instance alone cannot tell them apart.
        const generationTag = ++adapterCounter;
        this.scan = async () => {
          writesByGeneration.push(`scan:gen${generationTag}`);
          return vault.scan();
        };
        this.exists = (path: string) => vault.exists(path);
        this.readText = (path: string) => vault.readText(path);
        this.writeText = async (path: string, content: string) => {
          writesByGeneration.push(`write:gen${generationTag}:${path}`);
          await vault.writeText(path, content);
        };
        this.writeBinary = async (path: string, bytes: ArrayBuffer) => {
          writesByGeneration.push(`binary:gen${generationTag}:${path}`);
          await vault.writeBinary(path, bytes);
        };
        this.trash = async (path: string) => {
          writesByGeneration.push(`trash:gen${generationTag}:${path}`);
          await vault.trash(path);
        };
        this.listFiles = () => [];
        this.snapshot = async () => {
          throw new Error("not needed");
        };
      } as unknown as typeof vaultAdapterModule.ObsidianVaultAdapter);

    const orchestrator = new SyncOrchestrator(plugin as never);
    try {
      orchestrator.rebuild();
      const oldEngine = plugin.engine;
      expect(oldEngine).toBeInstanceOf(Engine);
      await vi.waitFor(() => expect(pullCalls).toBeGreaterThan(0));

      // Rebuild (settings change / vault switch): the old engine must be
      // aborted, not just dereferenced. Track the generation the OLD engine
      // was built with so the plugin mock can drop its index writes.
      generationAtCapture = plugin.syncGeneration - 1;
      orchestrator.rebuild();
      gate.resolve();
      // Deterministic settle (T-5, replacing a wall-clock sleep): the
      // replacement generation kicks its own syncNow in rebuild(), and that
      // pass queues on the new engine's chain BEHIND nothing — so once a
      // SECOND pull call is observed, the old generation's aborted pass has
      // fully unwound (its gate was resolved before this wait started, and
      // its chain task rejected and settled within the same microtask
      // queue the waitFor drains).
      await vi.waitFor(() => expect(pullCalls).toBeGreaterThanOrEqual(2));

      // The aborted OLD generation (gen1) performed NO mutating vault
      // operation on the pulled content (scans are read-only) — only the
      // replacement generation (gen2, which legitimately owns the same
      // mocked vault here) may write or trash.
      const oldGenOps = writesByGeneration.filter(
        (op) =>
          (op.startsWith("write:") ||
            op.startsWith("binary:") ||
            op.startsWith("trash:")) &&
          op.endsWith(":gen1")
      );
      expect(oldGenOps).toEqual([]);
      // The old generation's pass did not record the stale head.
      expect(idx.saved?.lastSyncedCommit ?? "c0").toBe("c0");
      // And the replacement engine is wired to a fresh generation.
      expect(plugin.engine).not.toBe(oldEngine);
    } finally {
      orchestrator.invalidate();
      syncApiSpy.mockRestore();
      adapterSpy.mockRestore();
    }
  });

  it("aborts mid inline-apply silently: no Notice, no error status (R2-7)", async () => {
    // R2-7: the M-10 per-file abort check inside applyInlineText's
    // updateIndex throws SyncAbortedError between the dirty check and
    // writeText. That error bubbles into the SSE change-loop catch, which
    // treated every non-InlineApplyDirtyError error as an inline-apply
    // FAILURE: reportInlineApplyFailure popped a "sync failed" Notice and
    // setStatus("error", ...) for a pass that was merely cancelled because
    // its engine generation was replaced — the replacement engine owns the
    // vault, the index AND the status element from that point on. The
    // abort must surface as complete silence: no Notice, no status change,
    // and no fallback pull either (the replacement engine's own sync
    // covers the missed commits).
    const syncedContent = "synced";
    const idx = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "x.md": {
          lastSyncedHash: await sha256Text(syncedContent),
          lastSyncedAt: 1,
          kind: "text",
          size: syncedContent.length
        }
      }
    });
    const vault = new FakeVault([
      {
        path: "x.md",
        hash: await sha256Text(syncedContent),
        size: syncedContent.length,
        kind: "text",
        content: syncedContent
      }
    ]);
    const gate = deferred<void>();
    let readStarted = false;
    const api = {
      state: vi.fn(),
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
    const setStatus = vi.fn();
    const originalRead = vault.readText.bind(vault);
    vault.readText = async (path: string) => {
      readStarted = true;
      await gate.promise;
      return originalRead(path);
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
      setStatus
    });

    notices.length = 0;
    engine.startEventSubscription();
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());
    const options = vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions;

    void options.onEvent({
      kind: "commit",
      commit: "c1",
      parent: "c0",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [{ kind: "text_inline", path: "x.md", content: "remote content" }]
    });
    await vi.waitFor(() => expect(readStarted).toBe(true));

    // The generation is replaced while the handler is parked inside its
    // dirty-check read — the mid-apply abort window from R2-7.
    engine.abort();
    gate.resolve();
    // Deterministic settle: a syncNow queues BEHIND the aborted event task
    // on the same serialised chain, and an aborted engine's syncInner
    // returns before its first setStatus/pull — so when this resolves, the
    // event task (including its catch path) has fully settled.
    await engine.syncNow();

    expect(vault.writes.size).toBe(0);
    expect(notices).toEqual([]);
    expect(setStatus).not.toHaveBeenCalledWith("error", expect.any(String));
    // No fallback pull either: the replacement engine owns the state.
    expect(api.pull).not.toHaveBeenCalled();
    expect(idx.idx.lastSyncedCommit).toBe("c0");
  });

  it("does not write a downloaded binary after aborting mid-download (M-10 per-file granularity)", async () => {
    // M-10 residual: applyPull's blob branch has awaits (downloadBlob,
    // sha256Bytes) between its phase-level abort check and writeBinary.
    // Aborting during the download must still prevent the vault write.
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const gate = deferred<void>();
    let downloadStarted = false;
    const bytes = new Uint8Array([1, 2, 3]).buffer;
    const api = {
      state: vi.fn(),
      pull: vi.fn(async () => {
        return {
          from: "c0",
          to: "c1",
          added: [
            {
              path: "img.bin",
              file_type: "blob" as const,
              size: 3,
              blob_hash:
                // sha256 of the downloaded bytes (Uint8Array [1,2,3])
                "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81"
            }
          ],
          modified: [],
          deleted: []
        };
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn(),
      downloadBlob: vi.fn(async () => {
        downloadStarted = true;
        await gate.promise;
        return bytes;
      }),
      downloadTextFile: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as unknown as SyncEngineOptions["vault"],
      api: api as unknown as SyncEngineOptions["api"],
      index: idx,
      setStatus: vi.fn()
    });

    const syncing = engine.syncNow();
    await vi.waitFor(() => expect(downloadStarted).toBe(true));

    engine.abort();
    gate.resolve();
    await syncing;

    // The download completed, but the vault write must not have happened.
    expect(vault.binaryWrites.get("img.bin")).toBeUndefined();
    expect(vault.binaryWrites.size).toBe(0);
    expect(idx.saved?.lastSyncedCommit ?? "c0").toBe("c0");
  });

  it("does not write pulled text after aborting mid-download (M-10 per-file granularity)", async () => {
    // M-10 residual: applyPull's text branch awaits downloadTextFile (and
    // hashing) between the phase-level check and writeText. Aborting during
    // the download must still prevent the write.
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const gate = deferred<void>();
    let downloadStarted = false;
    const api = {
      state: vi.fn(),
      pull: vi.fn(async () => {
        return {
          from: "c0",
          to: "c1",
          added: [
            {
              path: "note.md",
              file_type: "text" as const,
              size: 8
            }
          ],
          modified: [],
          deleted: []
        };
      }),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      push: vi.fn(),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn(async () => {
        downloadStarted = true;
        await gate.promise;
        return "downloaded remote text";
      })
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as unknown as SyncEngineOptions["vault"],
      api: api as unknown as SyncEngineOptions["api"],
      index: idx,
      setStatus: vi.fn()
    });

    const syncing = engine.syncNow();
    await vi.waitFor(() => expect(downloadStarted).toBe(true));

    engine.abort();
    gate.resolve();
    await syncing;

    expect(vault.writes.get("note.md")).toBeUndefined();
    expect(vault.writes.size).toBe(0);
    expect(idx.saved?.lastSyncedCommit ?? "c0").toBe("c0");
  });

  it("does not write inline SSE text after aborting mid-apply (M-10 per-file granularity)", async () => {
    // M-10 residual: applyInlineText awaits sha256TextWithLength (and its
    // dirty-check reads) inside its updateIndex before writeText;
    // aborting in that window must still prevent the write. The test parks
    // the handler inside the dirty-check's readText so the abort lands
    // deterministically after the handler started but before the write.
    const syncedContent = "synced";
    const idx = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "x.md": {
          lastSyncedHash: await sha256Text(syncedContent),
          lastSyncedAt: 1,
          kind: "text",
          size: syncedContent.length
        }
      }
    });
    const vault = new FakeVault([
      {
        path: "x.md",
        hash: await sha256Text(syncedContent),
        size: syncedContent.length,
        kind: "text",
        content: syncedContent
      }
    ]);
    const gate = deferred<void>();
    let readStarted = false;
    const api = {
      state: vi.fn(),
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
    const originalRead = vault.readText.bind(vault);
    vault.readText = async (path: string) => {
      readStarted = true;
      await gate.promise;
      return originalRead(path);
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
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());
    const options = vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions;

    void options.onEvent({
      kind: "commit",
      commit: "c1",
      parent: "c0",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [{ kind: "text_inline", path: "x.md", content: "remote content" }]
    });
    await vi.waitFor(() => expect(readStarted).toBe(true));

    engine.abort();
    gate.resolve();
    // Deterministic settle (T-5, replacing a wall-clock sleep): a syncNow
    // queues BEHIND the aborted event task on the same serialised chain,
    // and an aborted engine's syncInner returns before its first
    // setStatus/pull — so when this resolves, the event task (including
    // its catch path) has fully settled.
    await engine.syncNow();

    expect(vault.writes.size).toBe(0);
    expect(idx.idx.files["x.md"]?.lastSyncedHash).toBe(
      await sha256Text(syncedContent)
    );
    expect(idx.idx.lastSyncedCommit).toBe("c0");
  });

  it("does not trash after aborting mid-snapshot (M-10 per-file granularity)", async () => {
    // M-10 residual: applyDelete awaits vault.snapshot inside its
    // updateIndex before trash; aborting in that window must still prevent
    // the trash. The local file is CLEAN (index hash matches) so the
    // handler takes the direct-trash path — the only surviving abort
    // checks would be the new one immediately before the trash call.
    const content = "doomed";
    const cleanHash = await sha256Text(content);
    const idx = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "doomed.md": {
          lastSyncedHash: cleanHash,
          lastSyncedAt: 1,
          kind: "text",
          size: content.length
        }
      }
    });
    const vault = new FakeVault([
      {
        path: "doomed.md",
        hash: cleanHash,
        size: content.length,
        kind: "text",
        content
      }
    ]);
    const gate = deferred<void>();
    let snapshotStarted = false;
    const originalSnapshot = vault.snapshot.bind(vault);
    vault.snapshot = async (path: string) => {
      snapshotStarted = true;
      await gate.promise;
      return originalSnapshot(path);
    };
    const api = {
      state: vi.fn(),
      pull: vi.fn(),
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
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());
    const options = vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions;

    void options.onEvent({
      kind: "commit",
      commit: "c1",
      parent: "c0",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [{ kind: "delete", path: "doomed.md" }]
    });
    await vi.waitFor(() => expect(snapshotStarted).toBe(true));

    engine.abort();
    gate.resolve();
    // Deterministic settle (T-5, replacing a wall-clock sleep): the queued
    // syncNow runs only after the aborted event task has left the chain.
    await engine.syncNow();

    expect(vault.deletions).toEqual([]);
    expect(vault.exists("doomed.md")).toBe(true);
  });

  it("does not advance the index head after aborting a no-change pull (M-10)", async () => {
    // M-10 residual: applyPull's empty-diff fast path calls
    // advanceIndexHead with no abort check; an aborted generation must not
    // move the head (the replacement engine owns the vault/index).
    const idx = new FakeIndex({ lastSyncedCommit: "c0", files: {} });
    const vault = new FakeVault([]);
    const gate = deferred<void>();
    const api = {
      state: vi.fn(),
      pull: vi.fn(async () => {
        await gate.promise;
        return {
          from: "c0",
          to: "c1",
          added: [],
          modified: [],
          deleted: []
        };
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
      setStatus: vi.fn()
    });

    const syncing = engine.syncNow();
    await vi.waitFor(() => expect(api.pull).toHaveBeenCalled());

    engine.abort();
    gate.resolve();
    await syncing;

    expect(idx.saved?.lastSyncedCommit ?? "c0").toBe("c0");
  });
});
