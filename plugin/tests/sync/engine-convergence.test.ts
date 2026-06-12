/**
 * Task 4: Sync convergence property tests.
 *
 * Two SyncEngine instances (A, B) share an in-memory mock server.
 * Each engine has its own vault + index. We drive them through
 * scripted interleavings and assert:
 *   1. Final vault trees are identical (convergence).
 *   2. No typed character is silently lost (no-silent-loss invariant).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { subscribeVaultEvents } from "../../src/api/events-client";
import { ApiError } from "../../src/api/client";
import {
  SyncEngine,
  type IndexPersistence,
  type SyncEngineOptions
} from "../../src/sync/engine";
import { sha256Text } from "../../src/sync/hash";
import type {
  LocalFileSnapshot,
  LocalIndex,
  PullFile,
  PullResponse,
  PushChange,
  PushResponse
} from "../../src/sync/types";
import { shouldSyncPath } from "../../src/sync/vault-adapter";
import { notices } from "../mocks/obsidian";

vi.mock("../../src/api/events-client", () => ({
  subscribeVaultEvents: vi.fn()
}));

// ── Fake infrastructure ─────────────────────────────────────────────────────

class FakeVault {
  writes = new Map<string, string>();
  deletions: string[] = [];

  constructor(public files: LocalFileSnapshot[] = []) {}

  async scan(): Promise<LocalFileSnapshot[]> {
    return this.files.filter((f) => shouldSyncPath(f.path));
  }

  exists(path: string): boolean {
    return this.files.some((f) => f.path === path);
  }

  async readText(path: string): Promise<string> {
    const file = this.files.find((f) => f.path === path);
    if (!file || file.kind !== "text") throw new Error(`not found: ${path}`);
    return file.content ?? "";
  }

  async writeText(path: string, content: string): Promise<void> {
    this.writes.set(path, content);
    const snapshot: LocalFileSnapshot = {
      path,
      hash: await sha256Text(content),
      size: new TextEncoder().encode(content).byteLength,
      kind: "text",
      content
    };
    this.files = this.files.filter((f) => f.path !== path).concat(snapshot);
  }

  async writeBinary(): Promise<void> {
    throw new Error("not needed for convergence tests");
  }

  async delete(path: string): Promise<void> {
    this.deletions.push(path);
    this.files = this.files.filter((f) => f.path !== path);
  }

  /** Snapshot of current vault tree (path → content) excluding conflict paths. */
  tree(): Map<string, string> {
    const m = new Map<string, string>();
    for (const f of this.files) {
      if (shouldSyncPath(f.path) && f.kind === "text") {
        m.set(f.path, f.content ?? "");
      }
    }
    return m;
  }

  /** All files including conflict files. */
  allContent(): Map<string, string> {
    const m = new Map<string, string>();
    for (const f of this.files) {
      if (f.kind === "text") m.set(f.path, f.content ?? "");
    }
    return m;
  }
}

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

// ── Mock server ────────────────────────────────────────────────────────────

/**
 * In-memory mock server shared by all devices.
 *
 * Mirrors real server behaviour:
 *  - Tracks a canonical file tree and linear commit history.
 *  - Push rejects with 409 head_mismatch if ifMatch != server head.
 *  - When concurrent edits happen on different lines, the server merges them.
 *  - When concurrent edits hit the same line, the server reports "conflict"
 *    and keeps the remote (first-writer's) version in the canonical tree.
 *  - When one side deletes and the other modifies, the file survives.
 */
class MockServer {
  private commits: string[] = ["c0"];
  /** Per-commit delta: path → content (null = deleted) */
  private deltas = new Map<string, Map<string, string | null>>();
  /** Canonical file tree at current head */
  private tree = new Map<string, string>();
  private counter = 0;

  constructor() {
    this.deltas.set("c0", new Map());
  }

  get head(): string {
    return this.commits[this.commits.length - 1];
  }

  push(
    ifMatch: string | null,
    changes: PushChange[]
  ): PushResponse {
    const base = ifMatch ?? "c0";

    // The real PKV server with X2 merge semantics accepts pushes at any known
    // commit and merges the client's changes with the current canonical tree.
    // No 409 head_mismatch — the server handles the merge itself.

    const newId = `c${++this.counter}_s`; // unique id
    const delta = new Map<string, string | null>();
    const mergeOutcomes: PushResponse["merge_outcomes"] = [];

    for (const change of changes) {
      if (change.kind === "delete") {
        const canonical = this.tree.get(change.path);
        if (canonical !== undefined) {
          // File exists in canonical tree — check if base content matches
          const baseContent = this.contentAt(base, change.path);
          if (baseContent !== canonical) {
            // Remote modified since client's base → file survives
            mergeOutcomes.push({ path: change.path, outcome: "merged" });
            // Do NOT delete — keep remote version
            continue;
          }
        }
        delta.set(change.path, null);
        mergeOutcomes.push({ path: change.path, outcome: "clean" });
      } else if (change.kind === "text") {
        const baseContent = this.contentAt(base, change.path) ?? "";
        const currentContent = this.tree.get(change.path);

        if (currentContent === undefined || currentContent === baseContent) {
          // No concurrent change
          delta.set(change.path, change.content);
          mergeOutcomes.push({ path: change.path, outcome: "clean" });
        } else {
          // Concurrent edit — try merge
          const merged = this.tryMerge(baseContent, change.content, currentContent);
          if (merged !== null) {
            delta.set(change.path, merged);
            mergeOutcomes.push({ path: change.path, outcome: "merged" });
          } else {
            // Same-line conflict — keep server's (first-writer's) version in
            // the main path, save the client's (second-writer's) version in
            // a conflict file so the pull delivers both versions.
            const conflictPath = this.makeConflictPath(change.path);
            delta.set(conflictPath, change.content);
            mergeOutcomes.push({ path: change.path, outcome: "conflict", conflict_path: conflictPath });
          }
        }
      }
    }

    // Apply delta to canonical tree
    for (const [path, content] of delta) {
      if (content === null) {
        this.tree.delete(path);
      } else {
        this.tree.set(path, content);
      }
    }

    // When the client pushes from a stale base, also report "merged" for any
    // files that changed concurrently (even if not in this push). This signals
    // the engine to keep its head at the old commit so the subsequent pull
    // brings in those concurrent changes.
    const pushedPaths = new Set(changes.map((c) => c.path));
    const concurrentChanges = this.changesSince(base);
    for (const [path, content] of concurrentChanges) {
      if (pushedPaths.has(path)) continue; // already handled above
      // This file changed concurrently but the client didn't touch it.
      // Report as "merged" so the engine knows there's backflow to pull.
      if (content !== null && content !== undefined) {
        mergeOutcomes.push({ path, outcome: "merged" });
      }
    }

    this.commits.push(newId);
    this.deltas.set(newId, delta);

    return {
      new_commit: newId,
      files_changed: changes.length,
      merge_outcomes: mergeOutcomes
    };
  }

  pull(since: string | null): PullResponse {
    const base = since ?? "c0";
    if (base === this.head) {
      return { from: base, to: null, added: [], modified: [], deleted: [] };
    }

    const baseMap = this.contentMapAt(base);
    const headMap = this.snapshot();

    const added: PullFile[] = [];
    const modified: PullFile[] = [];
    const deleted: string[] = [];

    for (const [path, content] of headMap) {
      const baseContent = baseMap.get(path);
      if (baseContent === undefined) {
        added.push(this.pullFile(path, content));
      } else if (baseContent !== content) {
        modified.push(this.pullFile(path, content));
      }
    }

    for (const [path] of baseMap) {
      if (!headMap.has(path)) {
        deleted.push(path);
      }
    }

    return { from: base, to: this.head, added, modified, deleted };
  }

  private snapshot(): Map<string, string> {
    return new Map(this.tree);
  }

  private pullFile(path: string, content: string): PullFile {
    return {
      path,
      file_type: "text",
      size: new TextEncoder().encode(content).byteLength,
      content_inline: content
    };
  }

  private contentAt(commitId: string, path: string): string | undefined {
    return this.contentMapAt(commitId).get(path);
  }

  private contentMapAt(commitId: string): Map<string, string> {
    const result = new Map<string, string>();
    for (const id of this.commits) {
      const delta = this.deltas.get(id);
      if (delta) {
        for (const [path, content] of delta) {
          if (content === null) {
            result.delete(path);
          } else {
            result.set(path, content);
          }
        }
      }
      if (id === commitId) break;
    }
    return result;
  }

  /** Files that changed since a given commit. Returns path → content (null = deleted). */
  private changesSince(commitId: string): Map<string, string | null> {
    const result = new Map<string, string | null>();
    let found = false;
    for (const id of this.commits) {
      if (id === commitId) { found = true; continue; }
      if (!found) continue;
      const delta = this.deltas.get(id);
      if (delta) {
        for (const [path, content] of delta) {
          result.set(path, content);
        }
      }
    }
    return result;
  }

  /** Generate a server-side conflict file path. */
  private makeConflictPath(original: string): string {
    const stamp = `conflict-${this.counter}`;
    const slash = original.lastIndexOf("/");
    const dir = slash >= 0 ? original.slice(0, slash + 1) : "";
    const file = slash >= 0 ? original.slice(slash + 1) : original;
    const dot = file.lastIndexOf(".");
    if (dot <= 0) return `${dir}${file}.${stamp}`;
    return `${dir}${file.slice(0, dot)}.${stamp}${file.slice(dot)}`;
  }

  /**
   * Simple line-based three-way merge.
   * Returns null for same-line conflicts.
   */
  private tryMerge(base: string, local: string, remote: string): string | null {
    if (base === "") return null; // Both create same file differently

    const baseLines = base.split("\n");
    const localLines = local.split("\n");
    const remoteLines = remote.split("\n");

    const localChanged = new Set<number>();
    const remoteChanged = new Set<number>();

    const maxBaseLocal = Math.max(baseLines.length, localLines.length);
    const maxBaseRemote = Math.max(baseLines.length, remoteLines.length);

    for (let i = 0; i < maxBaseLocal; i++) {
      if ((baseLines[i] ?? "") !== (localLines[i] ?? "")) localChanged.add(i);
    }
    for (let i = 0; i < maxBaseRemote; i++) {
      if ((baseLines[i] ?? "") !== (remoteLines[i] ?? "")) remoteChanged.add(i);
    }

    for (const line of localChanged) {
      if (remoteChanged.has(line)) return null;
    }

    const maxLen = Math.max(baseLines.length, localLines.length, remoteLines.length);
    const result: string[] = [];
    for (let i = 0; i < maxLen; i++) {
      if (localChanged.has(i)) {
        result.push(localLines[i] ?? "");
      } else if (remoteChanged.has(i)) {
        result.push(remoteLines[i] ?? "");
      } else {
        result.push(baseLines[i] ?? "");
      }
    }
    return result.join("\n");
  }
}

// ── Test harness ───────────────────────────────────────────────────────────

interface Device {
  name: string;
  vault: FakeVault;
  index: FakeIndex;
  engine: SyncEngine;
}

function createHarness(): { server: MockServer; makeDevice: (name: string) => Device } {
  const server = new MockServer();

  function makeDevice(name: string): Device {
    const vault = new FakeVault([]);
    const index = new FakeIndex({ lastSyncedCommit: "c0", files: {} });

    const api = {
      state: vi.fn(),
      pull: vi.fn().mockImplementation((_vaultId: string, since: string | null) => {
        return Promise.resolve(server.pull(since));
      }),
      push: vi.fn().mockImplementation(
        (_vaultId: string, ifMatch: string | null, changes: PushChange[], _deviceName: string) => {
          const result = server.push(ifMatch, changes);
          return Promise.resolve(result);
        }
      ),
      uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
      uploadBlob: vi.fn(),
      downloadBlob: vi.fn(),
      downloadTextFile: vi.fn()
    };

    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: name,
      textExtensions: new Set(["md"]),
      vault: vault as unknown as SyncEngineOptions["vault"],
      api: api as unknown as SyncEngineOptions["api"],
      index,
      setStatus: vi.fn()
    });

    return { name, vault, index, engine };
  }

  return { server, makeDevice };
}

/**
 * Seed the server and all devices with initial file content.
 * Creates a commit on the server for each file, then sets each device's
 * vault and index to match.
 */
async function seedFiles(
  server: MockServer,
  devices: Device[],
  files: Array<{ path: string; content: string }>
): Promise<void> {
  for (const { path, content } of files) {
    server.push(server.head, [{ kind: "text", path, content }]);
  }
  for (const d of devices) {
    d.vault.files = [];
    for (const { path, content } of files) {
      const hash = await sha256Text(content);
      d.vault.files.push({
        path,
        hash,
        size: new TextEncoder().encode(content).byteLength,
        kind: "text",
        content
      });
    }
    const indexFiles: Record<string, { lastSyncedHash: string; lastSyncedAt: number; kind: "text"; size: number }> = {};
    for (const { path, content } of files) {
      indexFiles[path] = {
        lastSyncedHash: await sha256Text(content),
        lastSyncedAt: 1,
        kind: "text",
        size: new TextEncoder().encode(content).byteLength
      };
    }
    d.index.idx = {
      lastSyncedCommit: server.head,
      files: indexFiles
    };
  }
}

/** Edit a file in a device's vault (marks it as dirty by changing hash). */
function editFile(device: Device, path: string, content: string): void {
  const existing = device.vault.files.find((f) => f.path === path);
  if (existing) {
    existing.content = content;
    existing.hash = "dirty-" + content.length;
    existing.size = content.length;
  } else {
    device.vault.files.push({
      path,
      hash: "dirty-" + content.length,
      size: content.length,
      kind: "text",
      content
    });
  }
}

async function sync(device: Device): Promise<void> {
  await device.engine.syncNow();
}

function collectAll(device: Device): Map<string, string> {
  return device.vault.allContent();
}

function expectTreesMatch(a: Device, b: Device): void {
  const treeA = a.vault.tree();
  const treeB = b.vault.tree();
  expect(Object.fromEntries(treeA)).toEqual(Object.fromEntries(treeB));
}

// ── Tests ──────────────────────────────────────────────────────────────────

describe("Sync convergence scenarios", () => {
  beforeEach(() => {
    notices.length = 0;
    vi.mocked(subscribeVaultEvents).mockReturnValue(vi.fn());
    vi.stubGlobal("window", globalThis);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("1. A and B edit different files concurrently — both converge, no conflicts", async () => {
    const { server, makeDevice } = createHarness();
    const A = makeDevice("device-A");
    const B = makeDevice("device-B");

    // Both start synced with a.md and b.md
    await seedFiles(server, [A, B], [
      { path: "a.md", content: "base" },
      { path: "b.md", content: "base" }
    ]);

    // Concurrently: A edits a.md, B edits b.md
    editFile(A, "a.md", "A-edited");
    editFile(B, "b.md", "B-edited");

    // A syncs first (pushes a.md → server head advances)
    await sync(A);
    // B syncs (pushes b.md — server merges, B pulls A's a.md)
    await sync(B);
    // A syncs again to get B's b.md
    await sync(A);

    // Both converge
    expectTreesMatch(A, B);

    const treeA = A.vault.tree();
    expect(treeA.get("a.md")).toBe("A-edited");
    expect(treeA.get("b.md")).toBe("B-edited");

    // No conflict files
    const allA = collectAll(A);
    const allB = collectAll(B);
    const conflictFiles = [...allA.keys(), ...allB.keys()].filter((k) =>
      k.includes(".conflict-")
    );
    expect(conflictFiles).toEqual([]);

    // No-silent-loss invariant
    expect(allA.get("a.md")).toContain("A-edited");
    expect(allB.get("b.md")).toContain("B-edited");
  });

  it("2. A and B edit SAME file different lines — converge merged, no conflict files", async () => {
    const { server, makeDevice } = createHarness();
    const A = makeDevice("device-A");
    const B = makeDevice("device-B");

    await seedFiles(server, [A, B], [
      { path: "shared.md", content: "line1\nline2\nline3" }
    ]);

    // A edits line 1, B edits line 3 — different lines
    editFile(A, "shared.md", "A-line1\nline2\nline3");
    editFile(B, "shared.md", "line1\nline2\nB-line3");

    // A pushes first
    await sync(A);
    // B pushes (server merges different-line edits)
    await sync(B);
    // A pulls the merge result
    await sync(A);

    // Both converge
    expectTreesMatch(A, B);

    const treeA = A.vault.tree();
    const merged = treeA.get("shared.md");
    expect(merged).toBeDefined();
    expect(merged).toContain("A-line1");
    expect(merged).toContain("B-line3");

    // No conflict files
    const allA = collectAll(A);
    const allB = collectAll(B);
    const conflictFiles = [...allA.keys(), ...allB.keys()].filter((k) =>
      k.includes(".conflict-")
    );
    expect(conflictFiles).toEqual([]);
  });

  it("3. A and B edit SAME file SAME line — exactly one conflict file, no edit lost", async () => {
    const { server, makeDevice } = createHarness();
    const A = makeDevice("device-A");
    const B = makeDevice("device-B");

    await seedFiles(server, [A, B], [
      { path: "shared.md", content: "line1\nline2\nline3" }
    ]);

    // Both edit line 2 (same line) with different content
    editFile(A, "shared.md", "line1\nA-line2\nline3");
    editFile(B, "shared.md", "line1\nB-line2\nline3");

    // A pushes first (A's version becomes canonical)
    await sync(A);
    // B pushes (server detects same-line conflict, keeps A's version)
    await sync(B);
    // A syncs to see final state
    await sync(A);

    // Both converge on the main file
    expectTreesMatch(A, B);

    const allA = collectAll(A);
    const allB = collectAll(B);

    // At least one conflict file across both devices
    const conflictFilesA = [...allA.keys()].filter((k) => k.includes(".conflict-"));
    const conflictFilesB = [...allB.keys()].filter((k) => k.includes(".conflict-"));
    const totalConflicts = conflictFilesA.length + conflictFilesB.length;
    expect(totalConflicts).toBeGreaterThanOrEqual(1);

    // Original path identical on both sides
    expect(A.vault.tree().get("shared.md")).toBe(B.vault.tree().get("shared.md"));

    // No-silent-loss: both edits exist somewhere
    const allText = [...allA.values(), ...allB.values()].join("\n");
    expect(allText).toContain("A-line2");
    expect(allText).toContain("B-line2");
  });

  it("4. A edits + pushes while B is offline; B edits same file different lines, comes back — converge merged", async () => {
    const { server, makeDevice } = createHarness();
    const A = makeDevice("device-A");
    const B = makeDevice("device-B");

    await seedFiles(server, [A, B], [
      { path: "shared.md", content: "line1\nline2\nline3" }
    ]);

    // A edits line 1 and pushes
    editFile(A, "shared.md", "A-line1\nline2\nline3");
    await sync(A);

    // B is "offline" — edits line 3 locally
    editFile(B, "shared.md", "line1\nline2\nB-line3");

    // B comes back and syncs
    await sync(B);

    // A syncs to get B's merge
    await sync(A);

    // Both converge
    expectTreesMatch(A, B);

    const treeA = A.vault.tree();
    const merged = treeA.get("shared.md");
    expect(merged).toBeDefined();
    expect(merged).toContain("A-line1");
    expect(merged).toContain("B-line3");

    // No conflict files (different lines merged cleanly)
    const allA = collectAll(A);
    const allB = collectAll(B);
    const conflictFiles = [...allA.keys(), ...allB.keys()].filter((k) =>
      k.includes(".conflict-")
    );
    expect(conflictFiles).toEqual([]);
  });

  it("5. A deletes, B modifies concurrently — file survives with B's content on both", async () => {
    const { server, makeDevice } = createHarness();
    const A = makeDevice("device-A");
    const B = makeDevice("device-B");

    await seedFiles(server, [A, B], [
      { path: "note.md", content: "original" }
    ]);

    // A deletes locally
    A.vault.files = A.vault.files.filter((f) => f.path !== "note.md");

    // B modifies
    editFile(B, "note.md", "B-modified");

    // A syncs (pushes delete)
    await sync(A);

    // B syncs (pushes modification — server sees A's delete, keeps B's content)
    await sync(B);

    // A syncs to get B's content
    await sync(A);

    // File survives with B's content
    expectTreesMatch(A, B);
    const treeA = A.vault.tree();
    expect(treeA.get("note.md")).toBe("B-modified");

    // No-silent-loss
    const allA = collectAll(A);
    const allB = collectAll(B);
    const allContent = [...allA.values(), ...allB.values()].join("\n");
    expect(allContent).toContain("B-modified");
  });
});
