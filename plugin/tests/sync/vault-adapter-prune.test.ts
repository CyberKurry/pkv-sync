import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { subscribeVaultEvents } from "../../src/api/events-client";
import {
  SyncEngine,
  type IndexPersistence,
  type SyncEngineOptions
} from "../../src/sync/engine";
import { sha256Text } from "../../src/sync/hash";
import type { LocalIndex } from "../../src/sync/types";
import { ObsidianVaultAdapter, type VaultHost } from "../../src/sync/vault-adapter";
import { TFile, TFolder } from "obsidian";
import { notices } from "../mocks/obsidian";

vi.mock("../../src/api/events-client", () => ({
  subscribeVaultEvents: vi.fn(),
  SseAuthError: class SseAuthError extends Error {}
}));

function tfile(path: string): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, {
    path,
    stat: { mtime: 1_700_000_000_000, ctime: 1_700_000_000_000, size: 5 }
  });
  return file;
}

function tfolder(path: string): TFolder {
  const folder = Object.create(TFolder.prototype) as TFolder;
  Object.assign(folder, { path, children: [] as (TFile | TFolder)[] });
  return folder;
}

/**
 * Vault fake with TWO parallel views: the vault cache (files/folders maps,
 * mirroring TFolder.children which never lists dot-entries) and the real
 * filesystem (disk, surfaced through adapter.list). Pruning must read the
 * disk view — a folder holding only `.nomedia` looks empty in the cache.
 */
class PruneFakeVault {
  files: TFile[] = [];
  folders = new Map<string, TFolder>();
  disk = new Map<string, { files: string[]; folders: string[] }>();
  configDir = ".obsidian";
  readonly adapter = {
    list: async (dir: string) => {
      const entry = this.disk.get(dir);
      return {
        files: entry?.files.map((p) => `${dir}/${p}`) ?? [],
        folders: entry?.folders.map((p) => `${dir}/${p}`) ?? []
      };
    }
  };

  getAbstractFileByPath(path: string): TFile | TFolder | null {
    return this.files.find((f) => f.path === path) ?? this.folders.get(path) ?? null;
  }

  // Engine-facing surface so the real adapter can scan this fake.
  getFiles(): TFile[] {
    return this.files;
  }

  async read(file: TFile): Promise<string> {
    return file.path;
  }
}

class PruneFakeFileManager {
  trashed: string[] = [];
  constructor(private vault: PruneFakeVault) {}

  async trashFile(file: TFile | TFolder): Promise<void> {
    this.trashed.push(file.path);
    this.vault.files = this.vault.files.filter((f) => f.path !== file.path);
    this.vault.folders.delete(file.path);
    // Keep the disk view in sync: a trashed entry disappears from disk too.
    const slash = file.path.lastIndexOf("/");
    if (slash < 0) return;
    const parent = this.vault.disk.get(file.path.slice(0, slash));
    const name = file.path.slice(slash + 1);
    if (parent) {
      parent.files = parent.files.filter((entry) => entry !== name);
      parent.folders = parent.folders.filter((entry) => entry !== name);
    }
  }
}

function makeAdapter(prune = true) {
  const vault = new PruneFakeVault();
  const fm = new PruneFakeFileManager(vault);
  const host = {
    vault: vault as unknown as VaultHost["vault"],
    fileManager: fm as unknown as VaultHost["fileManager"]
  };
  return {
    adapter: new ObsidianVaultAdapter(host as VaultHost, prune),
    vault,
    fm
  };
}

function seed(vault: PruneFakeVault, path: string, hidden = false) {
  vault.files.push(tfile(path));
  const segments = path.split("/");
  for (let i = 1; i < segments.length; i++) {
    const dir = segments.slice(0, i).join("/");
    if (!vault.folders.has(dir)) {
      vault.folders.set(dir, tfolder(dir));
      vault.disk.set(dir, { files: [], folders: [] });
    }
    if (i > 1) {
      const parent = segments.slice(0, i - 1).join("/");
      vault.disk.get(parent)!.folders.push(segments[i - 1]);
    }
  }
  const dir = segments.slice(0, -1).join("/");
  if (dir) vault.disk.get(dir)!.files.push(segments.at(-1)!);
  if (dir && hidden) vault.disk.get(dir)!.files.push(".nomedia");
}

describe("empty-folder prune (spec §4)", () => {
  beforeEach(() => {
    notices.length = 0;
    // debugLog (used by the prune error path, like every engine error path)
    // reads window.process.env; the node test environment has no window.
    vi.stubGlobal("window", globalThis);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("prunes the emptied parent chain upward, innermost first", async () => {
    const { adapter, vault, fm } = makeAdapter();
    seed(vault, "A/B/note.md");
    await adapter.trash("A/B/note.md");
    expect(fm.trashed).toEqual(["A/B/note.md", "A/B", "A"]);
  });

  it("stops at the first non-empty folder (visible sibling)", async () => {
    const { adapter, vault, fm } = makeAdapter();
    seed(vault, "A/B/note.md");
    seed(vault, "A/C/x.md");
    await adapter.trash("A/B/note.md");
    expect(fm.trashed).toEqual(["A/B/note.md", "A/B"]);
  });

  it("does NOT prune a folder that still holds a dot-entry on disk", async () => {
    const { adapter, vault, fm } = makeAdapter();
    seed(vault, "A/B/note.md", true);
    await adapter.trash("A/B/note.md");
    expect(fm.trashed).toEqual(["A/B/note.md"]);
  });

  it("never touches dot-root or config-dir chains", async () => {
    const { adapter, vault, fm } = makeAdapter();
    seed(vault, ".hidden/note.md");
    await adapter.trash(".hidden/note.md");
    expect(fm.trashed).toEqual([".hidden/note.md"]);
  });

  it("never prunes under a custom (non-dot) config dir", async () => {
    const { adapter, vault, fm } = makeAdapter();
    vault.configDir = "vault-config";
    seed(vault, "vault-config/plugins/x.md");
    await adapter.trash("vault-config/plugins/x.md");
    expect(fm.trashed).toEqual(["vault-config/plugins/x.md"]);
  });

  it("silently stops when the emptied parent is missing from the vault cache", async () => {
    const { adapter, vault, fm } = makeAdapter();
    seed(vault, "X/note.md");
    vault.folders.delete("X");
    await adapter.trash("X/note.md");
    expect(fm.trashed).toEqual(["X/note.md"]);
  });

  it("root-level file: no folder operations", async () => {
    const { adapter, vault, fm } = makeAdapter();
    seed(vault, "note.md");
    await adapter.trash("note.md");
    expect(fm.trashed).toEqual(["note.md"]);
  });

  it("prune failure never escapes trash()", async () => {
    const { adapter, vault, fm } = makeAdapter();
    seed(vault, "A/B/note.md");
    vi.spyOn(vault.adapter, "list").mockRejectedValue(new Error("io"));
    await expect(adapter.trash("A/B/note.md")).resolves.toBeUndefined();
    expect(fm.trashed).toEqual(["A/B/note.md"]);
  });

  it("a trashFile(folder) failure never escapes trash() either", async () => {
    const { adapter, vault, fm } = makeAdapter();
    seed(vault, "A/B/note.md");
    // Only the FOLDER trash fails; the file delete above it must still
    // succeed and be recordable.
    const original = fm.trashFile.bind(fm);
    vi.spyOn(fm, "trashFile").mockImplementation(async (file: TFile | TFolder) => {
      if (file instanceof TFolder) throw new Error("fs locked");
      await original(file);
    });
    await expect(adapter.trash("A/B/note.md")).resolves.toBeUndefined();
    expect(fm.trashed).toEqual(["A/B/note.md"]);
  });

  it("disabled switch keeps folders", async () => {
    const { adapter, vault, fm } = makeAdapter(false);
    seed(vault, "A/B/note.md");
    await adapter.trash("A/B/note.md");
    expect(fm.trashed).toEqual(["A/B/note.md"]);
    expect(notices).toHaveLength(0);
  });
});

describe("first-prune notice (spec §4.5)", () => {
  beforeEach(() => {
    notices.length = 0;
    vi.stubGlobal("window", globalThis);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("fires exactly once, naming the first pruned folder", async () => {
    const { adapter, vault } = makeAdapter();
    seed(vault, "A/B/note.md");
    seed(vault, "A/C/x.md");
    await adapter.trash("A/B/note.md");
    expect(notices).toHaveLength(1);
    expect(notices[0]).toContain("A/B");
    // A later prune on the same adapter instance stays silent.
    await adapter.trash("A/C/x.md");
    expect(notices).toHaveLength(1);
  });
});

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

describe("engine pull through the pruning adapter (spec §4.4)", () => {
  beforeEach(() => {
    vi.mocked(subscribeVaultEvents).mockReset();
    vi.stubGlobal("window", globalThis);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("records every pulled deletion even when the prune step fails inside trash()", async () => {
    const { adapter, vault, fm } = makeAdapter();
    seed(vault, "A/x.md");
    seed(vault, "B/y.md");
    // Every trash() now deletes the file fine but hits an io error while
    // listing the emptied parent. The engine records the path AFTER the
    // await (deleted.push comes later), so an escaping prune error would
    // lose bookkeeping and fail the whole pull (spec §4.4).
    vi.spyOn(vault.adapter, "list").mockRejectedValue(new Error("io"));

    const index = new FakeIndex({
      lastSyncedCommit: "c0",
      files: {
        "A/x.md": {
          lastSyncedHash: await sha256Text("A/x.md"),
          lastSyncedAt: 1,
          kind: "text",
          size: 5
        },
        "B/y.md": {
          lastSyncedHash: await sha256Text("B/y.md"),
          lastSyncedAt: 1,
          kind: "text",
          size: 5
        }
      }
    });
    const setStatus = vi.fn();
    const engine = new SyncEngine({
      vaultId: "v",
      deviceName: "d",
      textExtensions: new Set(["md"]),
      vault: adapter,
      api: {
        pull: vi.fn().mockResolvedValue({
          from: "c0",
          to: "c1",
          added: [],
          modified: [],
          deleted: ["A/x.md", "B/y.md"]
        }),
        uploadCheck: vi.fn().mockResolvedValue({ missing: [] }),
        uploadBlob: vi.fn(),
        push: vi.fn(),
        downloadBlob: vi.fn(),
        downloadTextFile: vi.fn()
      } as unknown as SyncEngineOptions["api"],
      index,
      setStatus
    });

    await expect(engine.syncNow()).resolves.toBeUndefined();

    // Both file deletes went through the SAME entry point; the prune
    // failures were swallowed inside trash().
    expect(fm.trashed).toEqual(["A/x.md", "B/y.md"]);
    // The pull still completed and recorded both deletions in the index.
    expect(setStatus).toHaveBeenCalledWith("connected");
    expect(index.saved?.lastSyncedCommit).toBe("c1");
    expect(index.saved?.files["A/x.md"]).toBeUndefined();
    expect(index.saved?.files["B/y.md"]).toBeUndefined();
  });
});
