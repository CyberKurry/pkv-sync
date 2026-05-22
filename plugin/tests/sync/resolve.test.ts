import { TFile } from "obsidian";
import { describe, expect, it } from "vitest";
import {
  acceptLocal,
  acceptRemote,
  type ConflictResolveVault,
  markMergeMarkersResolved
} from "../../src/sync/resolve";
import type { ConflictPair } from "../../src/sync/conflict-files";

function tfile(path: string): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, { path });
  return file;
}

class FakeVault implements ConflictResolveVault {
  deleted: string[] = [];
  modified: Array<{ path: string; content: string }> = [];
  created: Array<{ path: string; content: string }> = [];
  conflictFile = tfile("note.md.conflict-2026-05-16-143000-abc.md");
  private originalFile = tfile("note.md");

  constructor(
    private options: {
      originalExists?: boolean;
      conflictContent?: string;
    } = {}
  ) {}

  async read(_file: TFile): Promise<string> {
    return this.options.conflictContent ?? "remote content";
  }

  async delete(file: TFile): Promise<void> {
    this.deleted.push(file.path);
  }

  async modify(file: TFile, content: string): Promise<void> {
    this.modified.push({ path: file.path, content });
  }

  async create(path: string, content: string): Promise<TFile> {
    this.created.push({ path, content });
    return tfile(path);
  }

  getAbstractFileByPath(_path: string): TFile | null {
    return this.options.originalExists === false ? null : this.originalFile;
  }
}

function conflictPair(kind: ConflictPair["kind"]): ConflictPair {
  const conflictPath = "note.md.conflict-2026-05-16-143000-abc.md";
  return {
    originalPath: "note.md",
    conflictPath,
    kind,
    conflictFile: tfile(conflictPath)
  };
}

describe("acceptLocal", () => {
  it("only deletes the conflict file, original stays", async () => {
    const pair = conflictPair("remote_copy");
    const vault = new FakeVault();
    await acceptLocal(vault, pair);
    expect(vault.deleted).toContain(
      "note.md.conflict-2026-05-16-143000-abc.md"
    );
    expect(vault.deleted).toHaveLength(1);
  });
});

describe("acceptRemote", () => {
  it("overwrites original with conflict content and deletes conflict file", async () => {
    const pair = conflictPair("remote_copy");
    const vault = new FakeVault({
      originalExists: true,
      conflictContent: "remote content"
    });
    await acceptRemote(vault, pair);
    expect(vault.modified).toHaveLength(1);
    expect(vault.deleted).toContain(
      "note.md.conflict-2026-05-16-143000-abc.md"
    );
  });

  it("creates original file when it does not exist", async () => {
    const pair = conflictPair("remote_copy");
    const vault = new FakeVault({
      originalExists: false,
      conflictContent: "remote content"
    });
    await acceptRemote(vault, pair);
    expect(vault.created).toHaveLength(1);
    expect(vault.created[0].path).toBe("note.md");
    expect(vault.deleted).toContain(
      "note.md.conflict-2026-05-16-143000-abc.md"
    );
  });
});

describe("markMergeMarkersResolved", () => {
  it("refuses to resolve while merge markers remain", async () => {
    const pair = conflictPair("merge_markers");
    const vault = new FakeVault({
      conflictContent: [
        "<<<<<<< local",
        "local",
        "=======",
        "remote",
        ">>>>>>> remote"
      ].join("\n")
    });

    await expect(markMergeMarkersResolved(vault, pair)).resolves.toBe(false);
    expect(vault.modified).toHaveLength(0);
    expect(vault.deleted).toHaveLength(0);
  });

  it("copies resolved conflict content to original and deletes conflict file", async () => {
    const pair = conflictPair("merge_markers");
    const vault = new FakeVault({
      originalExists: true,
      conflictContent: "resolved content"
    });

    await expect(markMergeMarkersResolved(vault, pair)).resolves.toBe(true);
    expect(vault.modified).toEqual([
      { path: "note.md", content: "resolved content" }
    ]);
    expect(vault.deleted).toContain(
      "note.md.conflict-2026-05-16-143000-abc.md"
    );
  });
});
