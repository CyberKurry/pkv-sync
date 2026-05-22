import { TFile, TFolder } from "obsidian";
import { describe, expect, it } from "vitest";
import {
  detectObsidianSync,
  scanVaultForMigration
} from "../../src/sync/migrate-from-obsidian-sync";

function tfile(path: string, size: number): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, { path, stat: { size } });
  return file;
}

function tfolder(path: string): TFolder {
  const folder = Object.create(TFolder.prototype) as TFolder;
  Object.assign(folder, { path, children: [] });
  return folder;
}

class FakeVault {
  private readonly entries = new Map<string, TFile | TFolder>();
  private readonly text = new Map<string, string>();

  addFile(path: string, size: number, content = ""): void {
    this.entries.set(path, tfile(path, size));
    this.text.set(path, content);
  }

  addUnreadableFile(path: string, size: number): void {
    this.entries.set(path, tfile(path, size));
  }

  addFolder(path: string): void {
    this.entries.set(path, tfolder(path));
  }

  getFiles(): TFile[] {
    return [...this.entries.values()].filter(
      (entry): entry is TFile => entry instanceof TFile
    );
  }

  getAbstractFileByPath(path: string): TFile | TFolder | null {
    return this.entries.get(path) ?? null;
  }

  async read(file: TFile): Promise<string> {
    const content = this.text.get(file.path);
    if (content === undefined) throw new Error(`Unreadable file: ${file.path}`);
    return content;
  }
}

describe("detectObsidianSync", () => {
  it("detects likely Obsidian Sync usage when the sync directory exists", async () => {
    const vault = new FakeVault();
    vault.addFolder(".obsidian/sync");

    await expect(detectObsidianSync(vault)).resolves.toEqual({
      syncDirExists: true,
      syncPluginEnabled: false,
      likelyUsingSync: true
    });
  });

  it("detects likely Obsidian Sync usage when the community plugin is enabled", async () => {
    const vault = new FakeVault();
    vault.addFile(
      ".obsidian/community-plugins.json",
      17,
      JSON.stringify(["calendar", "obsidian-sync"])
    );

    await expect(detectObsidianSync(vault)).resolves.toEqual({
      syncDirExists: false,
      syncPluginEnabled: true,
      likelyUsingSync: true
    });
  });

  it("treats invalid or unreadable community plugin JSON as not enabled", async () => {
    const invalid = new FakeVault();
    invalid.addFile(".obsidian/community-plugins.json", 1, "{");
    const unreadable = new FakeVault();
    unreadable.addUnreadableFile(".obsidian/community-plugins.json", 1);

    await expect(detectObsidianSync(invalid)).resolves.toMatchObject({
      syncPluginEnabled: false,
      likelyUsingSync: false
    });
    await expect(detectObsidianSync(unreadable)).resolves.toMatchObject({
      syncPluginEnabled: false,
      likelyUsingSync: false
    });
  });
});

describe("scanVaultForMigration", () => {
  it("skips Obsidian Sync, private, device-specific, PKV plugin, and temporary files", () => {
    const vault = new FakeVault();
    vault.addFile("note.md", 12);
    vault.addFile("folder/image.png", 34);
    vault.addFile(".obsidian/sync/state.json", 1);
    vault.addFile(".obsidian/workspace.json", 2);
    vault.addFile(".obsidian/workspace-mobile.json", 3);
    vault.addFile(".obsidian/workspaces.json", 4);
    vault.addFile(".obsidian/cache", 5);
    vault.addFile(".obsidian/cache/db.json", 6);
    vault.addFile(".obsidian/plugins/pkv-sync/data.json", 7);
    vault.addFile(".trash/deleted.md", 8);
    vault.addFile("folder/write.lock", 9);
    vault.addFile("folder/upload.tmp", 10);
    vault.addFile(".DS_Store", 11);
    vault.addFile("Thumbs.db", 12);

    const result = scanVaultForMigration(vault);

    expect(result.files.map((file) => file.path)).toEqual([
      "note.md",
      "folder/image.png"
    ]);
    expect(result.skippedCount).toBe(12);
    expect(result.totalBytes).toBe(46);
  });
});
