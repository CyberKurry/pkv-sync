import { TFile, TFolder } from "obsidian";
import { describe, expect, it, vi } from "vitest";
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

class FakeVault {
  files = [
    tfile("note.md"),
    tfile(".obsidian/themes/custom.css"),
    tfile(".trash/deleted.md")
  ];
  folders = new Map<string, TFolder>();
  createdFolders: string[] = [];
  createdFiles = new Map<string, string>();

  getFiles(): TFile[] {
    return this.files;
  }

  getAbstractFileByPath(path: string): TFile | null {
    return this.files.find((file) => file.path === path) ?? null;
  }

  getFolderByPath(path: string): TFolder | null {
    return this.folders.get(path) ?? null;
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

  it("creates parent folders before writing a missing nested text file", async () => {
    const vault = new FakeVault();
    const adapter = new ObsidianVaultAdapter(vault as any);

    await adapter.writeText("folder/deeper/remote.md", "remote");

    expect(vault.createdFolders).toEqual(["folder", "folder/deeper"]);
    expect(vault.createdFiles.get("folder/deeper/remote.md")).toBe("remote");
  });

  it("reuses previous hashes for unchanged files and only reads changed files", async () => {
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

    expect(read).toHaveBeenCalledTimes(1);
    expect(read).toHaveBeenCalledWith(changed);
    expect(readBinary).not.toHaveBeenCalled();
    expect(snapshots).toEqual([
      {
        path: "unchanged.md",
        hash: "hash-from-index",
        size: 10,
        kind: "text",
        mtime: 1_700_000_000_000
      },
      {
        path: "unchanged.png",
        hash: "blob-hash-from-index",
        size: 4,
        kind: "blob",
        mtime: 1_700_000_000_010
      },
      {
        path: "changed.md",
        hash: expect.any(String),
        size: 19,
        kind: "text",
        content: "changed.md contents",
        mtime: 1_700_000_000_100
      }
    ]);
    expect(snapshots[2].hash).not.toBe("old-hash");
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

      expect(readPaths).not.toContain("unchanged.md");
      expect(maxActiveReads).toBeGreaterThan(1);
      expect(maxActiveReads).toBeLessThanOrEqual(8);

      for (let attempt = 0; attempt <= changedFiles.length && !scanSettled; attempt += 1) {
        await resolvePendingReads();
      }

      const snapshots = await scanPromise;

      expect(read).toHaveBeenCalledTimes(changedFiles.length);
      expect(readPaths).toEqual(changedFiles.map((file) => file.path));
      expect(snapshots.map((snapshot) => snapshot.path)).toEqual([
        "unchanged.md",
        ...changedFiles.map((file) => file.path)
      ]);
      expect(snapshots[0]).toEqual({
        path: "unchanged.md",
        hash: "hash-from-index",
        size: 10,
        kind: "text",
        mtime: 1_700_000_000_000
      });
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
