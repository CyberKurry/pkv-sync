import { TFile } from "obsidian";
import { describe, expect, it } from "vitest";
import {
  deleteConflictFiles,
  isConflictPath,
  listConflictFiles
} from "../../src/sync/conflict-files";

function tfile(path: string): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, { path });
  return file;
}

class FakeVault {
  deleted: string[] = [];

  constructor(private files: TFile[]) {}

  getFiles(): TFile[] {
    return this.files;
  }

  async delete(file: TFile): Promise<void> {
    this.deleted.push(file.path);
    this.files = this.files.filter((candidate) => candidate.path !== file.path);
  }
}

describe("conflict file helpers", () => {
  it("matches only PKV Sync conflict filenames", () => {
    expect(isConflictPath("note.conflict-2026-04-29-143022-laptop.md")).toBe(
      true
    );
    expect(
      isConflictPath("folder/image.conflict-2026-04-29-120000-phone.png")
    ).toBe(true);
    expect(isConflictPath("my.conflict-resolution-notes.md")).toBe(false);
    expect(isConflictPath("folder.conflict-backup/note.md")).toBe(false);
  });

  it("lists and deletes conflict files in one pass", async () => {
    const vault = new FakeVault([
      tfile("note.md"),
      tfile("note.conflict-2026-04-29-143022-laptop.md"),
      tfile("my.conflict-resolution-notes.md"),
      tfile("folder/image.conflict-2026-04-29-120000-phone.png")
    ]);

    expect(listConflictFiles(vault).map((file) => file.path)).toEqual([
      "note.conflict-2026-04-29-143022-laptop.md",
      "folder/image.conflict-2026-04-29-120000-phone.png"
    ]);

    await expect(deleteConflictFiles(vault)).resolves.toBe(2);
    expect(vault.deleted).toEqual([
      "note.conflict-2026-04-29-143022-laptop.md",
      "folder/image.conflict-2026-04-29-120000-phone.png"
    ]);
  });
});
