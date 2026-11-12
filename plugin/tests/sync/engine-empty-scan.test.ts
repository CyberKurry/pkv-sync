import { beforeEach, describe, expect, it, vi } from "vitest";
import { subscribeVaultEvents } from "../../src/api/events-client";
import { SyncEngine, type SyncEngineOptions } from "../../src/sync/engine";
import type { LocalIndex } from "../../src/sync/types";

vi.mock("../../src/api/events-client", () => ({
  subscribeVaultEvents: vi.fn(),
  SseAuthError: class SseAuthError extends Error {}
}));

const labels = {
  emptyScanHeld: "vault appears empty; deletions held"
} as unknown as SyncEngineOptions["labels"];

function makeIndex(count: number): LocalIndex {
  const files: LocalIndex["files"] = {};
  for (let i = 0; i < count; i++) {
    files[`notes/n${i}.md`] = {
      lastSyncedHash: `h${i}`,
      lastSyncedAt: 1,
      kind: "text",
      size: 1
    };
  }
  return { lastSyncedCommit: "c0", files };
}

function makeEngine(opts: {
  scanEmpty: boolean;
  index: LocalIndex;
}) {
  const state = { scanEmpty: opts.scanEmpty };
  const push = vi.fn(
    async (
      _vaultId: string,
      _ifMatch: string | null,
      _changes: unknown[],
      _deviceName: string
    ) => ({ new_commit: "c1" })
  );
  const setStatus = vi.fn();
  const vault = {
    configDir: ".obsidian",
    scan: async () =>
      state.scanEmpty
        ? []
        : [ { path: "present.md", hash: "x", size: 1, kind: "text" as const, content: "x" } ],
    exists: () => false,
    trash: vi.fn()
  };
  const engine = new SyncEngine({
    vaultId: "v", deviceName: "d",
    textExtensions: new Set(["md"]),
    vault: vault as unknown as SyncEngineOptions["vault"],
    api: {
      pull: vi.fn(async () => ({ from: "c1", to: "c1", added: [], modified: [], deleted: [] })),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(), push,
      downloadBlob: vi.fn(), downloadTextFile: vi.fn()
    } as unknown as SyncEngineOptions["api"],
    index: {
      loadIndex: async () => opts.index,
      updateIndex: async () => undefined
    },
    setStatus, labels
  });
  return {
    engine,
    push,
    setStatus,
    setScanEmpty: (value: boolean) => {
      state.scanEmpty = value;
    }
  };
}

describe("SyncEngine empty-scan breaker (D3)", () => {
  beforeEach(() => {
    vi.mocked(subscribeVaultEvents).mockReset();
    vi.stubGlobal("window", globalThis);
  });

  it("holds ALL deletions when the scan lists zero files but the index is populated", async () => {
    const { engine, push, setStatus } = makeEngine({ scanEmpty: true, index: makeIndex(300) });
    await engine.syncNow();
    expect(push).not.toHaveBeenCalled();
    expect(setStatus).toHaveBeenCalledWith("error", "vault appears empty; deletions held");
  });

  it("still reports connected when the index is also empty (fresh vault)", async () => {
    const { engine, setStatus } = makeEngine({ scanEmpty: true, index: makeIndex(0) });
    await engine.syncNow();
    expect(setStatus).toHaveBeenCalledWith("connected");
  });

  it("one-shot override lets the deletes through exactly once", async () => {
    const { engine, push, setStatus } = makeEngine({ scanEmpty: true, index: makeIndex(3) });
    engine.allowEmptyScanDeletesOnce();
    await engine.syncNow();
    expect(push).toHaveBeenCalledTimes(1);
    const changes = push.mock.calls[0][2] as { kind: string }[];
    expect(changes).toHaveLength(3);
    expect(changes.every((c) => c.kind === "delete")).toBe(true);
    expect(setStatus).toHaveBeenCalledWith("connected");
  });
});

describe("SyncEngine empty-scan confirm escape hatch (D3)", () => {
  beforeEach(() => {
    vi.mocked(subscribeVaultEvents).mockReset();
    vi.stubGlobal("window", globalThis);
  });

  it("reports the held deletion count after a held pass", async () => {
    const { engine } = makeEngine({ scanEmpty: true, index: makeIndex(3) });
    await engine.syncNow();
    expect(engine.heldDeletionCount()).toBe(3);
  });

  it("resets the held count to 0 after a non-held pass", async () => {
    const { engine, setScanEmpty } = makeEngine({
      scanEmpty: true,
      index: makeIndex(3)
    });
    await engine.syncNow();
    expect(engine.heldDeletionCount()).toBe(3);
    setScanEmpty(false);
    await engine.syncNow();
    expect(engine.heldDeletionCount()).toBe(0);
  });

  it("applies the one-shot override exactly once: the next pass holds again", async () => {
    const { engine, push } = makeEngine({ scanEmpty: true, index: makeIndex(3) });
    engine.allowEmptyScanDeletesOnce();
    await engine.syncNow();
    await engine.syncNow();
    expect(push).toHaveBeenCalledTimes(1);
    expect(engine.heldDeletionCount()).toBe(3);
  });
});
