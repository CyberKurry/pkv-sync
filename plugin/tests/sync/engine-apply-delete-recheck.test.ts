import { describe, expect, it, vi } from "vitest";
import { SyncEngine, type SyncEngineOptions } from "../../src/sync/engine";
import type { LocalIndex } from "../../src/sync/types";

/**
 * R12-3: applyDelete had a snapshot→trash TOCTOU window. An edit landing
 * between the (clean) snapshot and the vault.trash was destroyed — the
 * conflict sidecar only held the pre-edit bytes. The fix mirrors
 * applyInlineText: re-snapshot before trashing; a changed hash skips the
 * trash (the full pull reconciles, and the just-changed content stays on
 * disk for it to see).
 */
describe("R12-3 applyDelete TOCTOU recheck", () => {
  it("does not trash when the file changed between snapshot and trash", async () => {
    const trash = vi.fn(async () => {});
    const writeConflict = vi.fn(async () => {});
    const snapshots = [
      { hash: "h-clean" }, // first snapshot: clean vs index
      { hash: "h-edited" }, // recheck: user edit landed in the window
    ];
    let calls = 0;
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      vault: {
        exists: () => true,
        snapshot: vi.fn(async () => {
          const s = snapshots[Math.min(calls, snapshots.length - 1)];
          calls += 1;
          return { kind: "text", path: "a.md", hash: s.hash, mtime: 0, size: 1 };
        }),
        trash,
      },
      index: {
        updateIndex: vi.fn(
          async (fn: (i: LocalIndex) => Promise<LocalIndex>) =>
            fn({ lastSyncedCommit: null, files: { "a.md": { lastSyncedHash: "h-clean", lastSyncedAt: 0, kind: "text", size: 1 } } }),
        ),
      },
      // The dirty path recheck route uses the engine's own writeConflict;
      // expose a spy seam via the engine after construction instead.
    } as unknown as SyncEngineOptions);
    // Route the engine's writeConflict to the spy.
    (
      engine as unknown as { writeConflict: (p: string, s: unknown) => Promise<void> }
    ).writeConflict = writeConflict as never;

    const applyDelete = (
      engine as unknown as { applyDelete: (p: string, c: string) => Promise<void> }
    ).applyDelete.bind(engine);
    await applyDelete("a.md", "commit-1");

    expect(calls).toBe(2); // snapshot + recheck
    expect(trash).not.toHaveBeenCalled(); // the edit stopped the trash
  });

  it("trashes when the recheck agrees the file is clean", async () => {
    const trash = vi.fn(async () => {});
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      vault: {
        exists: () => true,
        snapshot: vi.fn(async () => ({
          kind: "text",
          path: "a.md",
          hash: "h-clean",
          mtime: 0,
          size: 1,
        })),
        trash,
      },
      index: {
        updateIndex: vi.fn(
          async (fn: (i: LocalIndex) => Promise<LocalIndex>) =>
            fn({ lastSyncedCommit: null, files: { "a.md": { lastSyncedHash: "h-clean", lastSyncedAt: 0, kind: "text", size: 1 } } }),
        ),
      },
    } as unknown as SyncEngineOptions);
    const applyDelete = (
      engine as unknown as { applyDelete: (p: string, c: string) => Promise<void> }
    ).applyDelete.bind(engine);
    await applyDelete("a.md", "commit-2");
    expect(trash).toHaveBeenCalled();
  });
});
