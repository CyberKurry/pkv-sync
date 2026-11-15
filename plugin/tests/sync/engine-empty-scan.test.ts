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

describe("SyncEngine empty-scan confirmation is scoped to one pass (R15-7)", () => {
  beforeEach(() => {
    vi.mocked(subscribeVaultEvents).mockReset();
    vi.stubGlobal("window", globalThis);
  });

  it("a confirmation armed during an in-flight pass does not leak into a later one", async () => {
    // The confirmation is consumed at the TOP of syncInner. If the user
    // clicks "Confirm empty-vault deletions" while a pass is already in
    // flight, syncNow coalesces onto that pass — which passed the
    // consumption point long ago — so the armed flag survives the pass it
    // was meant for and is lifted by the NEXT one, however much later.
    // The user then gets a whole-vault deletion they never confirmed for
    // that moment, which is precisely what the breaker exists to prevent.
    let releasePull: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => {
      releasePull = resolve;
    });
    const { engine, push, setScanEmpty } = makeEngine({
      scanEmpty: true,
      index: makeIndex(3),
      // The in-flight pass parks here, well past the override consumption.
      pullImpl: async () => {
        await gate;
        return { from: "c1", to: "c1", added: [], modified: [], deleted: [] };
      }
    });

    // Pass A starts and reaches its pull.
    const passA = engine.syncNow();
    await Promise.resolve();
    await Promise.resolve();

    // The user confirms while A is in flight; syncNow coalesces onto A.
    engine.allowEmptyScanDeletesOnce();
    void engine.syncNow();
    releasePull?.();
    await passA;

    // The click must be honoured by a pass that STARTS after it. That pass
    // is scheduled by the completion hook, so it is already the engine's
    // active pass by the time `passA` resolves — awaiting it directly is
    // the deterministic way to observe it, and with the defect there is no
    // such pass to await (activeSync is null and nothing is pushed).
    // The completion hook schedules the pass the click asked for. `passA`
    // resolves before that pass finishes (it is deliberately fire-and-forget
    // so a completed pass is not delayed), so wait for its observable
    // effect instead of guessing at a tick count.
    await vi.waitFor(() => expect(push).toHaveBeenCalledTimes(1));
    await vi.waitFor(() => expect(engine.emptyScanConfirmPending()).toBe(false));

    const deleteBatches = () =>
      push.mock.calls.filter((call) =>
        (call[2] as { kind: string }[]).every((change) => change.kind === "delete")
      );
    expect(deleteBatches()).toHaveLength(1);
    expect(engine.heldDeletionCount()).toBe(0);

    // The confirmation is spent. A later pass holds again, so the flag
    // never survives into an unrelated pass.
    setScanEmpty(true);
    await engine.syncNow();
    expect(deleteBatches()).toHaveLength(1);
    expect(engine.heldDeletionCount()).toBe(3);
  });

  it("does not let the confirmation survive past the pass that consumed it", async () => {
    // The narrowest form of the guard: arm, run the pass that lifts it,
    // then run another pass without confirming. No delete may happen in
    // the second one.
    const { engine, push } = makeEngine({ scanEmpty: true, index: makeIndex(3) });
    engine.allowEmptyScanDeletesOnce();
    await engine.syncNow();
    expect(push).toHaveBeenCalledTimes(1);
    await engine.syncNow();
    expect(push).toHaveBeenCalledTimes(1);
    expect(engine.heldDeletionCount()).toBe(3);
  });

  it("still applies a confirmation that arrives when no pass is in flight", async () => {
    // The fix must not break the feature it protects.
    const { engine, push } = makeEngine({ scanEmpty: true, index: makeIndex(2) });
    engine.allowEmptyScanDeletesOnce();
    await engine.syncNow();
    const changes = push.mock.calls[0][2] as { kind: string }[];
    expect(changes).toHaveLength(2);
    expect(changes.every((change) => change.kind === "delete")).toBe(true);
  });
});
