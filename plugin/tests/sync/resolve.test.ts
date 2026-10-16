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

class FakeFileManager {
  trashed: string[] = [];

  async trashFile(file: TFile): Promise<void> {
    this.trashed.push(file.path);
  }
}

class FakeVault implements ConflictResolveVault {
  modified: Array<{ path: string; content: string }> = [];
  created: Array<{ path: string; content: string }> = [];
  readonly fileManager = new FakeFileManager();
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

function host(vault: FakeVault) {
  return { vault, fileManager: vault.fileManager };
}

function conflictPair(
  kind: ConflictPair["kind"],
  sidecarRole: ConflictPair["sidecarRole"] = "local"
): ConflictPair {
  const conflictPath =
    sidecarRole === "remote"
      ? "note.md.conflict-2026-05-16-143000-remote.md"
      : "note.md.conflict-2026-05-16-143000-abc.md";
  return {
    originalPath: "note.md",
    conflictPath,
    kind,
    conflictFile: tfile(conflictPath),
    sidecarRole
  };
}

describe("acceptLocal", () => {
  it("restores the local edit from a local-role sidecar onto the original (V1)", async () => {
    // Common shape: local dirty vs remote change — the sidecar holds the
    // user's ONLY local copy. "Keep local" must preserve it on the
    // original path, not trash it.
    const pair = conflictPair("remote_copy", "local");
    const vault = new FakeVault({
      originalExists: true,
      conflictContent: "my local draft"
    });
    await acceptLocal(host(vault), pair);
    expect(vault.modified).toEqual([
      { path: "note.md", content: "my local draft" }
    ]);
    expect(vault.fileManager.trashed).toContain(pair.conflictPath);
    expect(vault.fileManager.trashed).toHaveLength(1);
  });

  it("keeps the local deletion for a remote-role sidecar: sidecar removed, original untouched", async () => {
    // Local-deleted vs remote-modified: "keep local" means the file stays
    // deleted here; the remote copy leaves with the sidecar.
    const pair = conflictPair("remote_copy", "remote");
    const vault = new FakeVault({
      originalExists: false,
      conflictContent: "remote edit"
    });
    await acceptLocal(host(vault), pair);
    expect(vault.modified).toHaveLength(0);
    expect(vault.created).toHaveLength(0);
    expect(vault.fileManager.trashed).toContain(pair.conflictPath);
  });
});

describe("acceptRemote", () => {
  it("discards the local draft for a local-role sidecar: sidecar removed, disk keeps remote", async () => {
    // The original path already holds the remote version — accepting it
    // means removing the local sidecar, NOT republishing the draft.
    const pair = conflictPair("remote_copy", "local");
    const vault = new FakeVault({
      originalExists: true,
      conflictContent: "my local draft"
    });
    await acceptRemote(host(vault), pair);
    expect(vault.modified).toHaveLength(0);
    expect(vault.created).toHaveLength(0);
    expect(vault.fileManager.trashed).toContain(pair.conflictPath);
  });

  it("writes the remote content back for a remote-role sidecar", async () => {
    // Local-deleted vs remote-modified: "accept remote" resurrects the
    // file from the sidecar.
    const pair = conflictPair("remote_copy", "remote");
    const vault = new FakeVault({
      originalExists: false,
      conflictContent: "remote edit"
    });
    await acceptRemote(host(vault), pair);
    expect(vault.created).toEqual([{ path: "note.md", content: "remote edit" }]);
    expect(vault.fileManager.trashed).toContain(pair.conflictPath);
  });

  it("overwrites an existing original when accepting a remote-role sidecar", async () => {
    const pair = conflictPair("remote_copy", "remote");
    const vault = new FakeVault({
      originalExists: true,
      conflictContent: "remote content"
    });
    await acceptRemote(host(vault), pair);
    expect(vault.modified).toHaveLength(1);
    expect(vault.fileManager.trashed).toContain(pair.conflictPath);
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

    await expect(markMergeMarkersResolved(host(vault), pair)).resolves.toBe(false);
    expect(vault.modified).toHaveLength(0);
    expect(vault.fileManager.trashed).toHaveLength(0);
  });

  it("copies resolved conflict content to original and deletes conflict file", async () => {
    const pair = conflictPair("merge_markers");
    const vault = new FakeVault({
      originalExists: true,
      conflictContent: "resolved content"
    });

    await expect(markMergeMarkersResolved(host(vault), pair)).resolves.toBe(true);
    expect(vault.modified).toEqual([
      { path: "note.md", content: "resolved content" }
    ]);
    expect(vault.fileManager.trashed).toContain(pair.conflictPath);
  });
});
