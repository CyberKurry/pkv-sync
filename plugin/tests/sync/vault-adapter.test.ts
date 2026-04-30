import { TFile, TFolder } from "obsidian";
import { describe, expect, it } from "vitest";
import { ObsidianVaultAdapter, shouldSyncPath } from "../../src/sync/vault-adapter";

function tfile(path: string): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, { path });
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
    tfile(".obsidian/workspace.json"),
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
  it("scans syncable text files and skips Obsidian/trash internals", async () => {
    const adapter = new ObsidianVaultAdapter(new FakeVault() as any);

    const snapshots = await adapter.scan(new Set(["md"]));

    expect(snapshots).toHaveLength(1);
    expect(snapshots[0]).toMatchObject({
      path: "note.md",
      kind: "text",
      content: "hello",
      size: 5
    });
  });

  it("creates parent folders before writing a missing nested text file", async () => {
    const vault = new FakeVault();
    const adapter = new ObsidianVaultAdapter(vault as any);

    await adapter.writeText("folder/deeper/remote.md", "remote");

    expect(vault.createdFolders).toEqual(["folder", "folder/deeper"]);
    expect(vault.createdFiles.get("folder/deeper/remote.md")).toBe("remote");
  });
});

describe("shouldSyncPath", () => {
  it("excludes .obsidian paths", () => {
    expect(shouldSyncPath(".obsidian/workspace.json")).toBe(false);
  });

  it("excludes .trash paths", () => {
    expect(shouldSyncPath(".trash/deleted.md")).toBe(false);
  });

  it("excludes conflict files", () => {
    expect(
      shouldSyncPath("note.conflict-2026-04-29-143022-iphone.md")
    ).toBe(false);
    expect(
      shouldSyncPath("folder/img.conflict-2026-04-29-120000-desktop.png")
    ).toBe(false);
  });

  it("allows normal files", () => {
    expect(shouldSyncPath("note.md")).toBe(true);
    expect(shouldSyncPath("folder/image.png")).toBe(true);
    expect(shouldSyncPath("folder.conflict-backup/note.md")).toBe(true);
  });
});
