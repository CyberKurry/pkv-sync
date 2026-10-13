import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { subscribeVaultEvents, type SubscribeOptions } from "../../src/api/events-client";
import {
  SyncEngine,
  type IndexPersistence,
  type SyncEngineOptions
} from "../../src/sync/engine";
import { sha256Text } from "../../src/sync/hash";
import type { LocalFileSnapshot, LocalIndex, PullResponse } from "../../src/sync/types";
import { shouldSyncPath } from "../../src/sync/vault-adapter";

vi.mock("../../src/api/events-client", () => ({
  subscribeVaultEvents: vi.fn()
}));

class FakeVault {
  configDir = ".obsidian";
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
      size: content.length,
      kind: "text",
      content
    };
    this.files = this.files.filter((file) => file.path !== path).concat(next);
  }
  async writeBinary(): Promise<void> {
    throw new Error("not needed");
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

describe("SyncEngine SSE/syncNow interleave (M-12)", () => {
  beforeEach(() => {
    vi.mocked(subscribeVaultEvents).mockReset();
    vi.stubGlobal("window", globalThis);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("does not let a parked pull write stale content over an already-applied event (M-12)", async () => {
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());
    // Deterministic form of the audit's interleave, at the vault-write
    // boundary. A syncNow pass is applying its C4 pull diff and parks
    // INSIDE vault.writeText (the write has not landed). While it is
    // parked, SSE delivers C4 then C5 for the same path; the event chain
    // applies both and leaves the vault + head at C5. Releasing the parked
    // pull then writes its stale C4 content OVER C5 and the vault regresses
    // to older content with the head claiming a commit whose content was
    // just destroyed. The invariant under test: while a sync pass is
    // mid-apply, event handling must not run concurrently (the audit's
    // remedy: the SSE chain and syncNow share one mutex).
    const contentC4 = "content at C4";
    const contentC5 = "content at C5";
    const idx = new FakeIndex({ lastSyncedCommit: "c3", files: {} });
    const vault = new FakeVault([]);
    const pullGate = deferred<void>();
    let pullParked = false;
    const pullResponse: PullResponse = {
      from: "c3",
      to: "c4",
      added: [],
      modified: [
        {
          path: "x.md",
          file_type: "text",
          size: contentC4.length,
          content_inline: contentC4
        }
      ],
      deleted: []
    };
    const api = {
      state: vi.fn(),
      pull: vi.fn(async () => {
        return pullResponse;
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

    // Park the FIRST vault write (the pull's C4 apply) until the test
    // releases it, so the events land while the pass is mid-apply.
    const originalWrite = vault.writeText.bind(vault);
    let firstWrite = true;
    vault.writeText = async (path: string, content: string) => {
      if (firstWrite) {
        firstWrite = false;
        pullParked = true;
        await pullGate.promise;
      }
      await originalWrite(path, content);
    };

    engine.startEventSubscription();
    const options = vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions;

    const syncing = engine.syncNow();
    await vi.waitFor(() => expect(pullParked).toBe(true));

    // Deliver C4 then C5 while the sync pass is parked mid-apply.
    options.onEvent({
      kind: "commit",
      commit: "c4",
      parent: "c3",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [{ kind: "text_inline", path: "x.md", content: contentC4 }]
    });
    options.onEvent({
      kind: "commit",
      commit: "c5",
      parent: "c4",
      source_device_id: "other",
      at: 1_700_000_001,
      changes: [{ kind: "text_inline", path: "x.md", content: contentC5 }]
    });
    // Let the event chain settle while the pass is still parked. Pre-fix,
    // both events fully land here (vault at C5, head at c5).
    await new Promise((resolve) => setTimeout(resolve, 40));
    // Release the parked pull. Pre-fix it then overwrote C5 with the stale
    // C4 diff; with the shared mutex the queued events only run AFTER the
    // pass completes (C4 first, then C5 re-applied on top).
    pullGate.resolve();
    await syncing;
    await vi.waitFor(() => {
      expect(vault.writes.get("x.md")).toBe(contentC5);
      expect(idx.idx.lastSyncedCommit).toBe("c5");
    });
  });
});
