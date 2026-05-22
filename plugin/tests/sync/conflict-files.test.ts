import { TFile } from "obsidian";
import { describe, expect, it } from "vitest";
import {
  deleteConflictFiles,
  findConflictPairsForPath,
  isConflictPath,
  listConflictFiles,
  originalPathFor,
  pairConflicts,
  pairConflictsWithKinds
} from "../../src/sync/conflict-files";

function tfile(path: string): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, { path });
  return file;
}

class FakeVault {
  deleted: string[] = [];

  constructor(
    private files: TFile[],
    private contentByPath: Record<string, string> = {}
  ) {}

  getFiles(): TFile[] {
    return this.files;
  }

  async delete(file: TFile): Promise<void> {
    this.deleted.push(file.path);
    this.files = this.files.filter(
      (candidate) => candidate.path !== file.path
    );
  }

  async read(file: TFile): Promise<string> {
    return this.contentByPath[file.path] ?? "";
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
  it("restores extension from generated conflict path", () => {
    expect(originalPathFor("SKILL.conflict-2026-05-19-143000-remote.md")).toBe(
      "SKILL.md"
    );
  });

  it("extracts original path from conflict markdown file", () => {
    expect(
      originalPathFor("note.conflict-2026-05-16-143000-abc.md")
    ).toBe("note.md");
  });

  it("extracts original path from conflict image file", () => {
    expect(
      originalPathFor("image.conflict-2026-05-16-143000-abc.png")
    ).toBe("image.png");
  });

  it("returns null for non-conflict file", () => {
    expect(originalPathFor("not-a-conflict.md")).toBeNull();
  });

  it("extracts original from nested path", () => {
    expect(
      originalPathFor(
        "folder/note.conflict-2026-05-16-143000-xyz.md"
      )
    ).toBe("folder/note.md");
  });
});

describe("pairConflicts", () => {
  it("pairs conflict files with their original paths", () => {
    const vault = new FakeVault([
      tfile("note.md"),
      tfile("note.conflict-2026-05-16-143000-abc.md"),
      tfile("folder/image.png"),
      tfile("folder/image.conflict-2026-05-16-143000-phone.png")
    ]);
    const pairs = pairConflicts(vault);
    expect(pairs).toHaveLength(2);
    expect(pairs[0].originalPath).toBe("note.md");
    expect(pairs[0].conflictPath).toBe(
      "note.conflict-2026-05-16-143000-abc.md"
    );
    expect(pairs[0].kind).toBe("remote_copy");
    expect(pairs[1].originalPath).toBe("folder/image.png");
    expect(pairs[1].conflictPath).toBe(
      "folder/image.conflict-2026-05-16-143000-phone.png"
    );
    expect(pairs[1].kind).toBe("remote_copy");
  });

  it("detects conflict files with git-style merge markers", async () => {
    const conflict = "note.conflict-2026-05-16-143000-abc.md";
    const vault = new FakeVault(
      [tfile("note.md"), tfile(conflict)],
      {
        [conflict]: [
          "<<<<<<< local",
          "local text",
          "=======",
          "remote text",
          ">>>>>>> remote"
        ].join("\n")
      }
    );

    await expect(pairConflictsWithKinds(vault)).resolves.toMatchObject([
      {
        originalPath: "note.md",
        conflictPath: conflict,
        kind: "merge_markers"
      }
    ]);
  });

  it("keeps remote copy kind when conflict content has no merge markers", async () => {
    const conflict = "note.conflict-2026-05-16-143000-abc.md";
    const vault = new FakeVault(
      [tfile("note.md"), tfile(conflict)],
      { [conflict]: "remote content" }
    );

    await expect(pairConflictsWithKinds(vault)).resolves.toMatchObject([
      {
        originalPath: "note.md",
        conflictPath: conflict,
        kind: "remote_copy"
      }
    ]);
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

describe("findConflictPairsForPath", () => {
  it("finds conflicts from either the original file or the generated conflict file", () => {
    const conflict = "folder/note.conflict-2026-05-16-143000-phone.md";
    const vault = new FakeVault([
      tfile("folder/note.md"),
      tfile(conflict),
      tfile("other.md")
    ]);

    expect(findConflictPairsForPath(vault, "folder/note.md")).toMatchObject([
      {
        originalPath: "folder/note.md",
        conflictPath: conflict
      }
    ]);
    expect(findConflictPairsForPath(vault, conflict)).toMatchObject([
      {
        originalPath: "folder/note.md",
        conflictPath: conflict
      }
    ]);
    expect(findConflictPairsForPath(vault, "other.md")).toEqual([]);
  });
});
