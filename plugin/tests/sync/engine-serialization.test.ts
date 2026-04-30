import { describe, expect, it, vi } from "vitest";
import { SyncEngine, type IndexPersistence } from "../../src/sync/engine";
import type { LocalIndex } from "../../src/sync/types";

class FakeIndex implements IndexPersistence {
  constructor(public idx: LocalIndex) {}

  async loadIndex(): Promise<LocalIndex> {
    return this.idx;
  }

  async saveIndex(index: LocalIndex): Promise<void> {
    this.idx = index;
  }
}

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

describe("SyncEngine serialization", () => {
  it("coalesces concurrent syncNow calls into one sync pass", async () => {
    const gate = deferred();
    const api = {
      state: vi.fn(async () => {
        await gate.promise;
        return { current_head: null, changed_since: false };
      }),
      pull: vi.fn(),
      uploadCheck: vi.fn(),
      uploadBlob: vi.fn(),
      push: vi.fn(),
      downloadBlob: vi.fn()
    };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: { scan: vi.fn(async () => []) } as any,
      api: api as any,
      index: new FakeIndex({ lastSyncedCommit: null, files: {} }),
      setStatus: vi.fn()
    });

    const first = engine.syncNow();
    const second = engine.syncNow();
    await Promise.resolve();

    expect(api.state).toHaveBeenCalledTimes(1);
    gate.resolve();
    await Promise.all([first, second]);
    expect(api.state).toHaveBeenCalledTimes(1);
  });
});
