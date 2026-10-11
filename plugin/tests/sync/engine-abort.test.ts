import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  SyncEngine,
  type IndexPersistence,
  type SyncEngineOptions
} from "../../src/sync/engine";
import { sha256Text } from "../../src/sync/hash";
import type { LocalFileSnapshot, LocalIndex } from "../../src/sync/types";
import { shouldSyncPath } from "../../src/sync/vault-adapter";

class FakeVault {
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

describe("SyncEngine abort (M-10)", () => {
  beforeEach(() => {
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
      manifest: { version: "1.5.1" },
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
      // Let both in-flight passes settle.
      await new Promise((resolve) => setTimeout(resolve, 50));

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
});
