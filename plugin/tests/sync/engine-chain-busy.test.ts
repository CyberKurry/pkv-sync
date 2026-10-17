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
  subscribeVaultEvents: vi.fn(),
  SseAuthError: class SseAuthError extends Error {}
}));

class FakeVault {
  configDir = ".obsidian";
  writes = new Map<string, string>();
  /** Ordered log of mutating operations ("write:<path>" / "trash:<path>"). */
  ops: string[] = [];
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
    this.ops.push(`write:${path}`);
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
    this.ops.push(`trash:${path}`);
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

describe("SyncEngine external syncNow vs in-flight event handler (M-12 follow-up)", () => {
  beforeEach(() => {
    vi.mocked(subscribeVaultEvents).mockReset();
    vi.stubGlobal("window", globalThis);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("queues an external syncNow behind a running event handler even with no pass in flight (M-12)", async () => {
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());
    // The bypass window this test pins down: an SSE event handler is
    // mid-apply (parked inside its writeText) with NO syncNow pass in
    // flight (activeSync === null). An EXTERNAL syncNow — a poll-timer
    // tick or a rebuild kick — arrives at that moment. It must queue
    // behind the event task, not run concurrently: running concurrently
    // is exactly the interleave class M-12 fixed (a pass writing a stale
    // diff over content an event was applying, or regressing the head
    // past a commit whose content had already been delivered). The old
    // `insideChain` flag was engine-global, so this external call read
    // nested=true and ran syncInner directly OFF the chain.
    const localContent = "local";
    const eventContent = "event-applied";
    const idx = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "x.md": {
          lastSyncedHash: await sha256Text(localContent),
          lastSyncedAt: 1,
          kind: "text",
          size: localContent.length
        }
      }
    });
    const vault = new FakeVault([
      {
        path: "x.md",
        hash: await sha256Text(localContent),
        size: localContent.length,
        kind: "text",
        content: localContent
      }
    ]);
    const writeGate = deferred<void>();
    let eventParked = false;
    const pullResponse: PullResponse = {
      from: "c0",
      to: "c1",
      added: [],
      modified: [],
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

    // Park the event handler's writeText (inside applyInlineText's atomic
    // updateIndex) until the test releases it.
    const originalWrite = vault.writeText.bind(vault);
    vault.writeText = async (path: string, content: string) => {
      await writeGate.promise;
      await originalWrite(path, content);
    };

    engine.startEventSubscription();
    const options = vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions;

    // Deliver the event; let its handler start and park inside writeText.
    void options.onEvent({
      kind: "commit",
      commit: "c1",
      parent: "c0",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [{ kind: "text_inline", path: "x.md", content: eventContent }]
    });
    await vi.waitFor(() => {
      // The handler is parked INSIDE the write (the op has not landed).
      expect(vault.writes.get("x.md")).toBeUndefined();
    });
    eventParked = true;

    // External syncNow (poll timer / rebuild kick) while the event task
    // holds the chain and activeSync === null.
    const external = engine.syncNow();
    // Deterministic window (T-5, replacing a wall-clock sleep): queueing
    // the pass onto the chain is pure microtask work — a fixed number of
    // microtask turns lets the (buggy) inline path run arbitrarily far
    // (all the way to its first await) while the event write stays parked.
    for (let i = 0; i < 10; i += 1) {
      await Promise.resolve();
    }

    // While the event is still parked, the external pass must NOT have
    // started applying anything: no pull call, no second vault op.
    expect(api.pull).not.toHaveBeenCalled();
    expect(vault.ops).toEqual([]);

    // Release the event write; both tasks settle in order.
    writeGate.resolve();
    await external;
    await vi.waitFor(() => expect(vault.writes.get("x.md")).toBe(eventContent));

    // The event's write landed, and only afterwards did the external pass
    // run its pull (the pull call count proves the pass started after the
    // event completed, not alongside it).
    expect(api.pull).toHaveBeenCalledTimes(1);
    expect(vault.ops).toEqual([`write:x.md`]);
    expect(idx.idx.lastSyncedCommit).toBe("c1");
    expect(eventParked).toBe(true);
  });

  it(
    "does not deadlock when an SSE fallback pass starts while an external pass is queued (M-12)",
    { timeout: 10_000 },
    async () => {
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());
    // The inverse interleave of the test above: an event handler runs a
    // fallback syncNow (nested, inline) AFTER an external syncNow has
    // already queued a pass behind the running event task. The nested
    // call must run its own inline pass — awaiting the QUEUED external
    // pass would wait for a task that cannot start until this very
    // handler finishes: a self-deadlock.
    const syncedContent = "synced";
    const eventContent = "event-applied";
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
    const writeGate = deferred<void>();
    const pullResponse: PullResponse = {
      from: "c0",
      to: "c1",
      added: [],
      modified: [],
      deleted: []
    };
    let pulls = 0;
    const api = {
      state: vi.fn(),
      pull: vi.fn(async () => {
        pulls += 1;
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

    const originalWrite = vault.writeText.bind(vault);
    vault.writeText = async (path: string, content: string) => {
      await writeGate.promise;
      await originalWrite(path, content);
    };

    engine.startEventSubscription();
    const options = vi.mocked(subscribeVaultEvents).mock.calls[0][0] as SubscribeOptions;

    // One event whose first change parks inside its write and whose
    // second change (text_ref) forces the fallback-pull path AFTER the
    // park — so an external pass can slip into the queue in between.
    void options.onEvent({
      kind: "commit",
      commit: "c1",
      parent: "c0",
      source_device_id: "other",
      at: 1_700_000_000,
      changes: [
        { kind: "text_inline", path: "x.md", content: eventContent },
        { kind: "text_ref", path: "big.md", size: 100 }
      ]
    });
    await vi.waitFor(() => {
      expect(vault.writes.get("x.md")).toBeUndefined();
    });

    // External pass queues behind the (parked) event task.
    const external = engine.syncNow();
    expect(pulls).toBe(0);

    // Release the parked write: the handler reaches its fallback
    // syncNow while the external pass is still queued.
    writeGate.resolve();

    // Both must settle — the nested fallback may not await the queued
    // external pass (that would never start under it). The explicit test
    // timeout (T-7) keeps the race decisive: under vitest's default 5s a
    // real deadlock would surface as a bare timeout instead of this
    // assertion's message.
    const outcome = await Promise.race([
      Promise.all([external]).then(() => "settled"),
      new Promise((resolve) => setTimeout(() => resolve("deadlock"), 3_000))
    ]);
    expect(outcome).toBe("settled");
    await vi.waitFor(() => {
      expect(vault.writes.get("x.md")).toBe(eventContent);
      expect(idx.idx.lastSyncedCommit).toBe("c1");
    });
    expect(pulls).toBeGreaterThanOrEqual(1);
  });
});
