import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { subscribeVaultEvents, type SubscribeOptions } from "../../src/api/events-client";
import {
  SyncEngine,
  type IndexPersistence,
  type SyncEngineOptions
} from "../../src/sync/engine";
import type { LocalIndex } from "../../src/sync/types";

vi.mock("../../src/api/events-client", () => ({
  subscribeVaultEvents: vi.fn()
}));

class FakeIndex implements IndexPersistence {
  constructor(public idx: LocalIndex) {}

  async loadIndex(): Promise<LocalIndex> {
    return this.idx;
  }

  async saveIndex(index: LocalIndex): Promise<void> {
    this.idx = index;
  }

  async updateIndex(
    updater: (index: LocalIndex) => LocalIndex | Promise<LocalIndex>
  ): Promise<void> {
    this.idx = await updater(this.idx);
  }
}

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

async function flushMicrotasks(times = 5): Promise<void> {
  for (let i = 0; i < times; i++) {
    await Promise.resolve();
  }
}

describe("SyncEngine serialization", () => {
  const subscribeVaultEventsMock = vi.mocked(subscribeVaultEvents);

  beforeEach(() => {
    vi.stubGlobal("window", globalThis);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  function baseEngineOptions(): SyncEngineOptions {
    return {
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: { scan: vi.fn(async () => []) } as any,
      api: {
        state: vi.fn(async () => ({ current_head: null, changed_since: false })),
        pull: vi.fn().mockResolvedValue({
          from: null,
          to: null,
          added: [],
          modified: [],
          deleted: []
        }),
        uploadCheck: vi.fn(),
        uploadBlob: vi.fn(),
        push: vi.fn(),
        downloadBlob: vi.fn(),
        downloadTextFile: vi.fn()
      } as any,
      index: new FakeIndex({ lastSyncedCommit: null, files: {} }),
      setStatus: vi.fn()
    };
  }

  it("delegates SSE reconnects to the events client", async () => {
    subscribeVaultEventsMock.mockReturnValue(vi.fn());
    const previousNodeEnv = process.env.NODE_ENV;
    process.env.NODE_ENV = "development";
    const debug = vi.spyOn(console, "debug").mockImplementation(() => undefined);

    const engine = new SyncEngine({
      ...baseEngineOptions(),
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_abc",
      token: "tok",
      deviceId: "dev",
      pluginVersion: "0.4.0"
    });

    try {
      engine.startEventSubscription();
      expect(subscribeVaultEventsMock).toHaveBeenCalledTimes(1);

      const firstSubscribe = subscribeVaultEventsMock.mock.calls[0]?.[0] as
        | SubscribeOptions
        | undefined;
      firstSubscribe?.onError(new Error("network down"));

      expect(subscribeVaultEventsMock).toHaveBeenCalledTimes(1);
      expect(debug).toHaveBeenCalledWith(
        "[pkv-sync] SSE event stream error; automatic reconnect will continue:",
        expect.any(Error)
      );

      engine.stopEventSubscription();
    } finally {
      process.env.NODE_ENV = previousNodeEnv;
      debug.mockRestore();
    }
  });

  it("coalesces concurrent syncNow calls into one sync pass", async () => {
    const gate = deferred();
    const api = {
      state: vi.fn(),
      pull: vi.fn(async () => {
        await gate.promise;
        return {
          from: null,
          to: null,
          added: [],
          modified: [],
          deleted: []
        };
      }),
      uploadCheck: vi.fn(),
      uploadBlob: vi.fn(),
      push: vi.fn(),
      downloadBlob: vi.fn()
    };
    const engine = new SyncEngine({
      ...baseEngineOptions(),
      vault: { scan: vi.fn(async () => []) } as any,
      api: api as any
    });

    const first = engine.syncNow();
    const second = engine.syncNow();
    await flushMicrotasks(10);

    expect(api.pull).toHaveBeenCalledTimes(1);
    gate.resolve();
    await Promise.all([first, second]);
    expect(api.pull).toHaveBeenCalledTimes(1);
    expect(api.state).not.toHaveBeenCalled();
  });
});
