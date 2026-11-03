import { describe, expect, it, vi } from "vitest";
import { SyncEngine, type SyncEngineOptions } from "../../src/sync/engine";
import { shouldSyncPath } from "../../src/sync/vault-adapter";
import type { IndexedFile, LocalFileSnapshot, LocalIndex } from "../../src/sync/types";

function indexed(hash: string): IndexedFile {
  return { lastSyncedHash: hash, lastSyncedAt: 0, kind: "text", size: 10 };
}

/**
 * R11-2 (scan↔delete-classification half): `shouldSyncPath` now refuses
 * literal percent-escaped separators (`a%2Fb.md`) — the server's R10-1
 * twin. But an index written by an OLDER plugin version may already hold
 * such a path (pushed before the server grew the rejection, or recorded
 * as a rejected push). scanPendingFrom classifies "in index, not in
 * current scan" as a LOCAL DELETE; if the escaped-separator refusal only
 * removes the file from the scan while the deletion filter still accepts
 * the path, the engine would EMIT A DELETE for a file the user still
 * has — the exact data-loss direction the R9-7 ruling forbids. Both
 * lists must consult the same rule.
 */
describe("scan/delete classification for escaped-separator paths (R11-2)", () => {
  function makeEngine(vaultFiles: string[], indexFiles: Record<string, IndexedFile>) {
    const snapshots: LocalFileSnapshot[] = vaultFiles.map((path) => ({
      path,
      hash: `hash-${path}`,
      size: 10,
      mtime: 0,
      kind: "text" as const,
      content: "x"
    }));
    const vault = {
      configDir: ".obsidian",
      // Mirror the real adapter: scan() filters through shouldSyncPath
      // (that filter is where R11-2 removes `a%2Fb.md` from the listing).
      scan: vi.fn(async () => snapshots.filter((s) => shouldSyncPath(s.path))),
      listFiles: vi.fn(() => snapshots),
      exists: vi.fn(() => false),
      readText: vi.fn(async () => "x"),
      readBinary: vi.fn(async () => new ArrayBuffer(0)),
      writeText: vi.fn(async () => undefined),
      writeBinary: vi.fn(async () => undefined),
      trash: vi.fn(async () => undefined),
      snapshot: vi.fn(async () => snapshots[0])
    };
    const pushed: { ifMatch: string | null; changes: unknown[] }[] = [];
    const index: LocalIndex = { lastSyncedCommit: "c0", files: indexFiles };
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: vault as unknown as SyncEngineOptions["vault"],
      api: {
        state: vi.fn(async () => ({ current_head: "c0", changed_since: false })),
        pull: vi.fn(async () => ({ from: null, to: null, added: [], modified: [], deleted: [] })),
        push: vi.fn(
          async (
            _vaultId: string,
            ifMatch: string | null,
            changes: unknown[]
          ) => {
            pushed.push({ ifMatch, changes });
            return { new_commit: "c1", files_changed: changes.length };
          }
        ),
        uploadCheck: vi.fn(async () => ({ missing: [] })),
        uploadBlob: vi.fn(async () => undefined),
        downloadBlob: vi.fn(async () => undefined),
        downloadTextFile: vi.fn(async () => "")
      } as unknown as SyncEngineOptions["api"],
      index: {
        loadIndex: async () => index,
        updateIndex: async (updater) => {
          const next = await updater(index);
          index.files = next.files;
        }
      },
      setStatus: vi.fn(),
      labels: {} as SyncEngineOptions["labels"],
      pushLabels: {} as SyncEngineOptions["pushLabels"],
      deviceId: "dev",
      serverUrl: "https://s.example.com",
      deploymentKey: "k",
      token: "t",
      pluginVersion: "1.7.3"
    });
    return { engine, pushed };
  }

  it("never classifies an escaped-separator index entry as a local delete", async () => {
    // The user still HAS `a%2Fb.md` (present in the vault listing) but the
    // scan refuses it (R11-2). An older version recorded it in the index.
    const { engine, pushed } = makeEngine(
      ["a%2Fb.md", "kept.md"],
      {
        "a%2Fb.md": indexed("hash-old"),
        "kept.md": indexed("hash-kept")
      }
    );

    await engine.syncNow();

    const deletePaths = pushed.flatMap(
      (p) =>
        (p.changes as Array<{ kind: string; path: string }>)
          .filter((c) => c.kind === "delete")
          .map((c) => c.path)
    );
    expect(deletePaths).not.toContain("a%2Fb.md");
    expect(deletePaths).toEqual([]);
  });

  it("still classifies a genuinely deleted file as a local delete", async () => {
    // Positive control: the guard must not swallow real deletions.
    const { engine, pushed } = makeEngine(["kept.md"], {
      "gone.md": indexed("hash-gone"),
      "kept.md": indexed("hash-kept")
    });

    await engine.syncNow();

    const deletePaths = pushed.flatMap(
      (p) =>
        (p.changes as Array<{ kind: string; path: string }>)
          .filter((c) => c.kind === "delete")
          .map((c) => c.path)
    );
    expect(deletePaths).toContain("gone.md");
  });
});
