import { TFile } from "obsidian";
import { describe, expect, it } from "vitest";
import {
  deleteConflictFiles,
  isConflictPath,
  listConflictFiles,
  originalPathFor,
  pairConflicts
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
    this.files = this.files.filter(
      (candidate) => candidate.path !== file.path
    );
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

describe("originalPathFor", () => {
  it("extracts original path from conflict markdown file", () => {
    expect(
      originalPathFor("note.md.conflict-2026-05-16-143000-abc.md")
    ).toBe("note.md");
  });

  it("extracts original path from conflict image file", () => {
    expect(
      originalPathFor("image.png.conflict-2026-05-16-143000-abc")
    ).toBe("image.png");
  });

  it("returns null for non-conflict file", () => {
    expect(originalPathFor("not-a-conflict.md")).toBeNull();
  });

  it("extracts original from nested path", () => {
    expect(
      originalPathFor(
        "folder/note.md.conflict-2026-05-16-143000-xyz.md"
      )
    ).toBe("folder/note.md");
  });
});

describe("pairConflicts", () => {
  it("pairs conflict files with their original paths", () => {
    const vault = new FakeVault([
      tfile("note.md"),
      tfile("note.md.conflict-2026-05-16-143000-abc.md"),
      tfile("folder/image.png"),
      tfile("folder/image.png.conflict-2026-05-16-143000-phone")
    ]);
    const pairs = pairConflicts(vault);
    expect(pairs).toHaveLength(2);
    expect(pairs[0].originalPath).toBe("note.md");
    expect(pairs[0].conflictPath).toBe(
      "note.md.conflict-2026-05-16-143000-abc.md"
    );
    expect(pairs[1].originalPath).toBe("folder/image.png");
    expect(pairs[1].conflictPath).toBe(
      "folder/image.png.conflict-2026-05-16-143000-phone"
    );
  });

  it("skips conflict files that do not match the pattern", () => {
    const vault = new FakeVault([
      tfile("note.md"),
      tfile("weird.conflict-file.md")
    ]);
    const pairs = pairConflicts(vault);
    expect(pairs).toHaveLength(0);
  });
});
