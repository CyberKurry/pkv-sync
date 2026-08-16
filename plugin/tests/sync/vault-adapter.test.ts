import { TFile, TFolder } from "obsidian";
import { describe, expect, it, vi } from "vitest";
import { sha256Text } from "../../src/sync/hash";
import {
  ObsidianVaultAdapter,
  shouldAcceptRemoteConflictPath,
  shouldSyncPath
} from "../../src/sync/vault-adapter";

function tfile(path: string, stat: Partial<TFile["stat"]> = {}): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, {
    path,
    stat: {
      mtime: 1_700_000_000_000,
      ctime: 1_700_000_000_000,
      size: 5,
      ...stat
    }
  });
  return file;
}

function tfolder(path: string): TFolder {
  const folder = Object.create(TFolder.prototype) as TFolder;
  Object.assign(folder, { path, children: [] });
  return folder;
}

function folderWithChildren(
  path: string,
  children: (TFile | TFolder)[]
): TFolder {
  const folder = tfolder(path);
  folder.children = children;
  return folder;
}

class FakeVault {
  files = [
    tfile("note.md"),
    tfile(".obsidian/themes/custom.css"),
    tfile(".trash/deleted.md")
  ];
  folders = new Map<string, TFolder>();
  createdFolders: string[] = [];
  createdFiles = new Map<string, string>();
  trashed: Array<{ path: string; system: boolean }> = [];

  getFiles(): TFile[] {
    return this.files;
  }

  getAbstractFileByPath(path: string): TFile | TFolder | null {
    const direct =
      this.files.find((file) => file.path === path) ?? this.folders.get(path);
    if (direct) return direct;
    const tree = new Map<string, TFile | TFolder>();
    for (const folder of this.folders.values()) {
      tree.set(folder.path, folder);
      this.indexTree(folder, tree);
    }
    return tree.get(path) ?? null;
  }

  private indexTree(node: TFolder, out: Map<string, TFile | TFolder>): void {
    for (const child of node.children) {
      if (child instanceof TFile || child instanceof TFolder) {
        out.set(child.path, child);
      }
      if (child instanceof TFolder) this.indexTree(child, out);
    }
  }

  async trash(file: TFile, system: boolean): Promise<void> {
    this.trashed.push({ path: file.path, system });
    this.files = this.files.filter((candidate) => candidate.path !== file.path);
  }

  async createFolder(path: string): Promise<TFolder> {
    const folder = tfolder(path);
    this.createdFolders.push(path);
    this.folders.set(path, folder);
    return folder;
  }

  async read(file: TFile): Promise<string> {
    return file.path === "note.md" ? "hello" : "ignored";
  }

  async readBinary(): Promise<ArrayBuffer> {
    return new Uint8Array([1, 2, 3, 4]).buffer;
  }

  async create(path: string, content: string): Promise<TFile> {
    const parent = path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "";
    if (parent && !this.folders.has(parent)) {
      throw new Error(`Missing parent folder ${parent}`);
    }
    const file = tfile(path);
    this.files.push(file);
    this.createdFiles.set(path, content);
    return file;
  }
}

describe("ObsidianVaultAdapter", () => {
  it("scans safe dot paths so sync policy can apply the allowlist", async () => {
    const adapter = new ObsidianVaultAdapter(new FakeVault() as any);

    const snapshots = await adapter.scan(new Set(["md", "css"]));

    expect(snapshots.map((snapshot) => snapshot.path)).toEqual([
      "note.md",
      ".obsidian/themes/custom.css"
    ]);
  });

  it("enumerates .obsidian files even when vault.getFiles() excludes them", async () => {
    const vault = new FakeVault();
    vault.files = [tfile("note.md")];
    vault.folders.set(
      ".obsidian",
      folderWithChildren(".obsidian", [
        folderWithChildren(".obsidian/plugins", [
          tfile(".obsidian/plugins/pkv-sync/main.js")
        ]),
        tfile(".obsidian/community-plugins.json")
      ])
    );

    const adapter = new ObsidianVaultAdapter(vault as any);
    const snapshots = await adapter.scan(new Set(["md", "json"]));

    const paths = snapshots.map((snapshot) => snapshot.path);
    expect(paths).toContain("note.md");
    expect(paths).toContain(".obsidian/plugins/pkv-sync/main.js");
    expect(paths).toContain(".obsidian/community-plugins.json");
  });

  it("drops content for files whose hash matches the previous index to bound memory", async () => {
    const unchanged = tfile("unchanged.md", { size: 3 });
    const changed = tfile("changed.md", { size: 8 });
    const vault = new FakeVault();
    vault.files = [unchanged, changed];
    vi.spyOn(vault, "read").mockImplementation(async (file: TFile) =>
      file.path === "unchanged.md" ? "abc" : "changed!"
    );
    const unchangedHash = await sha256Text("abc");
    const adapter = new ObsidianVaultAdapter(vault as any);

    const snapshots = await adapter.scan(new Set(["md"]), {
      lastSyncedCommit: "commit-1",
      files: {
        "unchanged.md": {
          lastSyncedHash: unchangedHash,
          lastSyncedAt: 0,
          lastSyncedMtime: unchanged.stat.mtime,
          kind: "text",
          size: unchanged.stat.size
        }
      }
    });

    const unchangedSnapshot = snapshots.find(
      (snapshot) => snapshot.path === "unchanged.md"
    )!;
    expect(unchangedSnapshot.content).toBeUndefined();
    expect(unchangedSnapshot.hash).toBe(unchangedHash);
    const changedSnapshot = snapshots.find(
      (snapshot) => snapshot.path === "changed.md"
    )!;
    expect(changedSnapshot.content).toBe("changed!");
  });

  it("can scan an initial sync without retaining file payloads", async () => {
    const file = tfile("initial.md", { size: 5 });
    const vault = new FakeVault();
    vault.files = [file];
    vi.spyOn(vault, "read").mockResolvedValue("hello");
    const adapter = new ObsidianVaultAdapter(vault as any);

    const snapshots = await adapter.scan(new Set(["md"]), undefined, {
      retainPayload: false
    });

    expect(snapshots).toHaveLength(1);
    expect(snapshots[0]).toMatchObject({
      path: "initial.md",
      kind: "text",
      size: 5,
      hash: expect.any(String)
    });
    expect(snapshots[0].content).toBeUndefined();
  });

  it("creates parent folders before writing a missing nested text file", async () => {
    const vault = new FakeVault();
    const adapter = new ObsidianVaultAdapter(vault as any);

    await adapter.writeText("folder/deeper/remote.md", "remote");

    expect(vault.createdFolders).toEqual(["folder", "folder/deeper"]);
    expect(vault.createdFiles.get("folder/deeper/remote.md")).toBe("remote");
  });

  it("snapshots files instead of trusting matching mtime and size", async () => {
    const unchanged = tfile("unchanged.md", {
      mtime: 1_700_000_000_000,
      size: 10
    });
    const unchangedBlob = tfile("unchanged.png", {
      mtime: 1_700_000_000_010,
      size: 4
    });
    const changed = tfile("changed.md", {
      mtime: 1_700_000_000_100,
      size: 19
    });
    const vault = new FakeVault();
    vault.files = [unchanged, unchangedBlob, changed];
    const read = vi.spyOn(vault, "read").mockImplementation(async (file: TFile) => {
      if (file.path === "unchanged.md") return "would hash differently";
      return "changed.md contents";
    });
    const readBinary = vi.spyOn(vault, "readBinary");
    const adapter = new ObsidianVaultAdapter(vault as any);

    const snapshots = await adapter.scan(new Set(["md"]), {
      lastSyncedCommit: "commit-1",
      files: {
        "unchanged.md": {
          lastSyncedHash: "hash-from-index",
          lastSyncedAt: 1_700_000_000_050,
          lastSyncedMtime: unchanged.stat.mtime,
          kind: "text",
          size: unchanged.stat.size
        },
        "unchanged.png": {
          lastSyncedHash: "blob-hash-from-index",
          lastSyncedAt: 1_700_000_000_050,
          lastSyncedMtime: unchangedBlob.stat.mtime,
          kind: "blob",
          size: unchangedBlob.stat.size
        },
        "changed.md": {
          lastSyncedHash: "old-hash",
          lastSyncedAt: 1_700_000_000_050,
          lastSyncedMtime: 1_700_000_000_000,
          kind: "text",
          size: changed.stat.size
        }
      }
    });

    expect(read).toHaveBeenCalledTimes(2);
    expect(read).toHaveBeenCalledWith(unchanged);
    expect(read).toHaveBeenCalledWith(changed);
    expect(readBinary).toHaveBeenCalledWith(unchangedBlob);
    expect(snapshots[0]).toMatchObject({
      path: "unchanged.md",
      size: "would hash differently".length,
      kind: "text",
      content: "would hash differently",
      mtime: 1_700_000_000_000
    });
    expect(snapshots[0].hash).not.toBe("hash-from-index");
    expect(snapshots[1]).toMatchObject({
      path: "unchanged.png",
      hash: expect.any(String),
      size: 4,
      kind: "blob",
      bytes: expect.any(ArrayBuffer),
      mtime: 1_700_000_000_010
    });
    expect(snapshots[1].hash).not.toBe("blob-hash-from-index");
    expect(snapshots[2]).toMatchObject({
      path: "changed.md",
      hash: expect.any(String),
      size: 19,
        kind: "text",
      content: "changed.md contents",
      mtime: 1_700_000_000_100
    });
    expect(snapshots[2].hash).not.toBe("old-hash");
  });

  it("rehashes files even when mtime and size match the previous index", async () => {
    const file = tfile("note.md", {
      mtime: 1_700_000_000_000,
      size: 5
    });
    const vault = new FakeVault();
    vault.files = [file];
    const read = vi.spyOn(vault, "read").mockResolvedValue("HELLO");
    const adapter = new ObsidianVaultAdapter(vault as any);

    const snapshots = await adapter.scan(new Set(["md"]), {
      lastSyncedCommit: "commit-1",
      files: {
        "note.md": {
          lastSyncedHash: "stale-hash",
          lastSyncedAt: 1_700_000_000_050,
          lastSyncedMtime: file.stat.mtime,
          kind: "text",
          size: file.stat.size
        }
      }
    });

    expect(read).toHaveBeenCalledWith(file);
    expect(snapshots[0]).toMatchObject({
      path: "note.md",
      size: 5,
      kind: "text",
      content: "HELLO",
      mtime: file.stat.mtime
    });
    expect(snapshots[0].hash).not.toBe("stale-hash");
  });

  it("snapshots changed files with bounded concurrency while preserving scan order", async () => {
    const unchanged = tfile("unchanged.md", {
      mtime: 1_700_000_000_000,
      size: 10
    });
    const changedFiles = Array.from({ length: 9 }, (_, index) =>
      tfile(`changed-${index + 1}.md`, {
        mtime: 1_700_000_000_100 + index,
        size: `changed ${index + 1}`.length
      })
    );
    const contents = new Map(
      changedFiles.map((file, index) => [file.path, `changed ${index + 1}`])
    );
    const vault = new FakeVault();
    vault.files = [unchanged, ...changedFiles];

    type PendingRead = {
      resolve(content: string): void;
    };

    const pendingReads = new Map<string, PendingRead>();
    const readPaths: string[] = [];
    let activeReads = 0;
    let maxActiveReads = 0;
    const read = vi.spyOn(vault, "read").mockImplementation((file: TFile) => {
      readPaths.push(file.path);
      activeReads += 1;
      maxActiveReads = Math.max(maxActiveReads, activeReads);

      let resolve!: (content: string) => void;
      const promise = new Promise<string>((settle) => {
        resolve = (content: string) => {
          activeReads -= 1;
          settle(content);
        };
      });
      pendingReads.set(file.path, { resolve });
      return promise;
    });
    const adapter = new ObsidianVaultAdapter(vault as any);

    let scanSettled = false;
    const scanPromise = adapter
      .scan(new Set(["md"]), {
        lastSyncedCommit: "commit-1",
        files: {
          "unchanged.md": {
            lastSyncedHash: "hash-from-index",
            lastSyncedAt: 1_700_000_000_050,
            lastSyncedMtime: unchanged.stat.mtime,
            kind: "text",
            size: unchanged.stat.size
          }
        }
      })
      .finally(() => {
        scanSettled = true;
      });

    const resolvePendingReads = async () => {
      for (const path of [...pendingReads.keys()].reverse()) {
        const pending = pendingReads.get(path);
        if (!pending) continue;
        pendingReads.delete(path);
        pending.resolve(contents.get(path) ?? "");
      }
      await new Promise((resolve) => setTimeout(resolve, 0));
    };

    try {
      await Promise.resolve();

      expect(readPaths).toContain("unchanged.md");
      expect(maxActiveReads).toBeGreaterThan(1);
      expect(maxActiveReads).toBeLessThanOrEqual(8);

      for (let attempt = 0; attempt <= changedFiles.length && !scanSettled; attempt += 1) {
        await resolvePendingReads();
      }

      const snapshots = await scanPromise;

      expect(read).toHaveBeenCalledTimes(changedFiles.length + 1);
      expect(readPaths).toEqual([
        "unchanged.md",
        ...changedFiles.map((file) => file.path)
      ]);
      expect(snapshots.map((snapshot) => snapshot.path)).toEqual([
        "unchanged.md",
        ...changedFiles.map((file) => file.path)
      ]);
      expect(snapshots[0]).toMatchObject({
        path: "unchanged.md",
        hash: expect.any(String),
        size: 0,
        kind: "text",
        content: "",
        mtime: 1_700_000_000_000
      });
      expect(snapshots[0].hash).not.toBe("hash-from-index");
      for (const [index, snapshot] of snapshots.slice(1).entries()) {
        expect(snapshot).toMatchObject({
          path: `changed-${index + 1}.md`,
          size: `changed ${index + 1}`.length,
          kind: "text",
          content: `changed ${index + 1}`,
          mtime: 1_700_000_000_100 + index
        });
      }
    } finally {
      for (let attempt = 0; attempt <= changedFiles.length && !scanSettled; attempt += 1) {
        await resolvePendingReads();
      }
      await scanPromise.catch(() => undefined);
    }
  });

  it("reports concurrent snapshot errors in scan order", async () => {
    const first = tfile("first.md");
    const second = tfile("second.md");
    const vault = new FakeVault();
    vault.files = [first, second];

    type PendingRead = {
      reject(error: Error): void;
    };

    const pendingReads = new Map<string, PendingRead>();
    vi.spyOn(vault, "read").mockImplementation((file: TFile) => {
      let reject!: (error: Error) => void;
      const promise = new Promise<string>((_resolve, rejectPromise) => {
        reject = rejectPromise;
      });
      pendingReads.set(file.path, { reject });
      return promise;
    });
    const adapter = new ObsidianVaultAdapter(vault as any);
    const scanError = adapter.scan(new Set(["md"])).then(
      () => new Error("scan unexpectedly resolved"),
      (error: Error) => error
    );

    await Promise.resolve();

    expect([...pendingReads.keys()]).toEqual(["first.md", "second.md"]);

    pendingReads.get("second.md")?.reject(new Error("second failed"));
    await Promise.resolve();
    pendingReads.get("first.md")?.reject(new Error("first failed"));

    const error = await scanError;
    expect(error.message).toBe("first failed");
  });

  it("rejects unsafe remote write paths before touching the vault", async () => {
    const vault = new FakeVault();
    const adapter = new ObsidianVaultAdapter(vault as any);

    await expect(
      adapter.writeText("folder/../.obsidian/plugins/evil/main.js", "evil")
    ).rejects.toThrow(/Unsafe sync path/);

    expect(vault.createdFolders).toEqual([]);
    expect(vault.createdFiles.size).toBe(0);
  });

  it("trashes deletions with the system trash instead of deleting permanently", async () => {
    const vault = new FakeVault();
    const adapter = new ObsidianVaultAdapter(vault as any);

    await adapter.trash("note.md");

    expect(vault.trashed).toEqual([{ path: "note.md", system: true }]);
    expect(vault.files.map((file) => file.path)).not.toContain("note.md");
  });

  it("rejects unsafe trash paths and ignores already-removed files", async () => {
    const vault = new FakeVault();
    const adapter = new ObsidianVaultAdapter(vault as any);

    await expect(adapter.trash("../outside.md")).rejects.toThrow(
      /Unsafe sync path/
    );
    await expect(adapter.trash("missing.md")).resolves.toBeUndefined();
    expect(vault.trashed).toEqual([]);
  });

  it("allows writing generated conflict files without making them syncable", async () => {
    const vault = new FakeVault();
    const adapter = new ObsidianVaultAdapter(vault as any);
    const conflict = "单片机/P155 T14.conflict-2026-05-12-204915-LJYsPredator.md";

    await adapter.writeText(conflict, "local version");

    expect(vault.createdFolders).toEqual(["单片机"]);
    expect(vault.createdFiles.get(conflict)).toBe("local version");
    expect(shouldSyncPath(conflict)).toBe(false);
  });
});

describe("shouldSyncPath", () => {
  it("allows .obsidian paths for the higher-level allowlist policy", () => {
    expect(shouldSyncPath(".obsidian/themes/custom.css")).toBe(true);
  });

  it("excludes .trash paths", () => {
    expect(shouldSyncPath(".trash/deleted.md")).toBe(false);
  });

  it("excludes git internals and unsafe traversal paths", () => {
    expect(shouldSyncPath(".git/config")).toBe(false);
    expect(shouldSyncPath("../outside.md")).toBe(false);
    expect(shouldSyncPath("folder/../outside.md")).toBe(false);
    expect(shouldSyncPath("/absolute.md")).toBe(false);
    expect(shouldSyncPath("C:/vault/note.md")).toBe(false);
    expect(shouldSyncPath("folder\\..\\outside.md")).toBe(false);
    expect(shouldSyncPath("%2e%2e/outside.md")).toBe(false);
    expect(shouldSyncPath("%252e%252e/outside.md")).toBe(false);
    expect(shouldSyncPath("%2eobsidian/plugins/evil/main.js")).toBe(false);
  });

  it("excludes conflict files", () => {
    expect(
      shouldSyncPath("note.conflict-2026-04-29-143022-iphone.md")
    ).toBe(false);
    expect(
      shouldSyncPath("folder/img.conflict-2026-04-29-120000-desktop.png")
    ).toBe(false);
  });

  it("accepts safe remote conflict files without making them scan-syncable", () => {
    expect(
      shouldAcceptRemoteConflictPath("note.conflict-2026-04-29-143022-iphone.md")
    ).toBe(true);
    expect(
      shouldAcceptRemoteConflictPath(
        "folder/img.conflict-2026-04-29-120000-desktop.png"
      )
    ).toBe(true);
    expect(
      shouldAcceptRemoteConflictPath(
        "folder/.git/note.conflict-2026-04-29-143022-x.md"
      )
    ).toBe(false);
    expect(
      shouldAcceptRemoteConflictPath(
        "folder/.trash/note.conflict-2026-04-29-143022-x.md"
      )
    ).toBe(false);
    expect(shouldAcceptRemoteConflictPath("../note.conflict-2026-04-29-143022-x.md")).toBe(
      false
    );
    expect(shouldAcceptRemoteConflictPath("note.md")).toBe(false);
  });

  it("allows normal files", () => {
    expect(shouldSyncPath("note.md")).toBe(true);
    expect(shouldSyncPath("folder/image.png")).toBe(true);
    expect(shouldSyncPath("folder.conflict-backup/note.md")).toBe(true);
    expect(shouldSyncPath("my.conflict-resolution-notes.md")).toBe(true);
  });
});
