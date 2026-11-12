import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { subscribeVaultEvents } from "../../src/api/events-client";
import { SyncEngine, type SyncEngineOptions } from "../../src/sync/engine";
import { sha256Text } from "../../src/sync/hash";
import type { LocalFileSnapshot, LocalIndex } from "../../src/sync/types";

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
  extraExcludeGlobs?: string[];
  scanFiles?: LocalFileSnapshot[];
  pullImpl?: () => Promise<unknown>;
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
  const onSyncSuccess = vi.fn();
  const pull = vi.fn(
    opts.pullImpl ??
      (async () => ({ from: "c1", to: "c1", added: [], modified: [], deleted: [] }))
  );
  /** Written files become scan-visible so a restoring pull un-empties the vault. */
  const files = new Map<string, LocalFileSnapshot>();
  const fallbackScan = (): LocalFileSnapshot[] =>
    opts.scanFiles ?? [
      { path: "present.md", hash: "x", size: 1, kind: "text", content: "x" }
    ];
  const vault = {
    configDir: ".obsidian",
    scan: async () =>
      files.size > 0 ? [...files.values()] : state.scanEmpty ? [] : fallbackScan(),
    exists: (path: string) => files.has(path),
    trash: vi.fn(),
    writeText: async (path: string, content: string): Promise<void> => {
      files.set(path, {
        path,
        hash: await sha256Text(content),
        size: content.length,
        kind: "text",
        content
      });
    }
  };
  const engine = new SyncEngine({
    vaultId: "v", deviceName: "d",
    textExtensions: new Set(["md"]),
    extraExcludeGlobs: opts.extraExcludeGlobs,
    vault: vault as unknown as SyncEngineOptions["vault"],
    api: {
      pull,
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(), push,
      downloadBlob: vi.fn(), downloadTextFile: vi.fn()
    } as unknown as SyncEngineOptions["api"],
    index: {
      loadIndex: async () => opts.index,
      updateIndex: async () => undefined
    },
    setStatus, labels, onSyncSuccess
  });
  return {
    engine,
    push,
    pull,
    setStatus,
    onSyncSuccess,
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

describe("SyncEngine empty-scan breaker review fixes", () => {
  beforeEach(() => {
    vi.mocked(subscribeVaultEvents).mockReset();
    vi.stubGlobal("window", globalThis);
  });

  it("keeps the confirmed override alive across a 409 head-mismatch retry in the same pass", async () => {
    const { engine, push, setStatus } = makeEngine({
      scanEmpty: true,
      index: makeIndex(3)
    });
    push.mockRejectedValueOnce(
      new ApiError(409, "head_mismatch", "server head moved")
    );
    engine.allowEmptyScanDeletesOnce();
    await engine.syncNow();
    // The first push was rejected 409; the retry reclassifies and must
    // still honor the confirmation — the override is pass-scoped.
    expect(push).toHaveBeenCalledTimes(2);
    const retryChanges = push.mock.calls[1][2] as { kind: string }[];
    expect(retryChanges).toHaveLength(3);
    expect(retryChanges.every((c) => c.kind === "delete")).toBe(true);
    expect(setStatus).toHaveBeenCalledWith("connected");
  });

  it("counts the scan PRE-matcher: an excluded-but-scanned file keeps the breaker off", async () => {
    const { engine, push, setStatus, onSyncSuccess } = makeEngine({
      scanEmpty: false,
      index: makeIndex(1),
      extraExcludeGlobs: ["excluded/**"],
      scanFiles: [
        { path: "excluded/keep.md", hash: "x", size: 1, kind: "text", content: "x" }
      ]
    });
    await engine.syncNow();
    // The scan lists one file, but the matcher excludes it — a post-filter
    // count would see zero and wrongly re-hold the indexed deletion.
    expect(push).toHaveBeenCalledWith(
      "v",
      "c0",
      [{ kind: "delete", path: "notes/n0.md" }],
      "d"
    );
    expect(setStatus).toHaveBeenCalledWith("connected");
    expect(onSyncSuccess).toHaveBeenCalled();
  });

  it("does not fire onSyncSuccess on a held pass, fires it on a normal one", async () => {
    const held = makeEngine({ scanEmpty: true, index: makeIndex(3) });
    await held.engine.syncNow();
    expect(held.onSyncSuccess).not.toHaveBeenCalled();

    const normal = makeEngine({ scanEmpty: false, index: makeIndex(0) });
    await normal.engine.syncNow();
    expect(normal.onSyncSuccess).toHaveBeenCalled();
  });

  it("a held pass whose pull restores content clears the held count and converges", async () => {
    let pullCalls = 0;
    const pullImpl = async () => {
      pullCalls += 1;
      return pullCalls === 1
        ? {
            from: "c0",
            to: "c1",
            added: [
              {
                path: "restored.md",
                file_type: "text" as const,
                size: 8,
                content_inline: "restored"
              }
            ],
            modified: [],
            deleted: []
          }
        : { from: "c1", to: "c1", added: [], modified: [], deleted: [] };
    };
    const { engine, setStatus } = makeEngine({
      scanEmpty: true,
      index: makeIndex(3),
      pullImpl
    });

    await engine.syncNow();
    // The pull's trailing classification saw the restored file, so the
    // held counter no longer pins a stale count.
    expect(engine.heldDeletionCount()).toBe(0);

    // With the vault no longer empty, the next pass needs no override.
    await engine.syncNow();
    expect(setStatus).toHaveBeenLastCalledWith("connected");
  });
});
