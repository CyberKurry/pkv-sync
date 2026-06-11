import { TFile } from "obsidian";
import { describe, expect, it, vi } from "vitest";
import { restoreFileToCommit } from "../../src/sync/restore";
import type { HistoryApi } from "../../src/api/history-client";

function tfile(path: string): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, { path });
  return file;
}

class FakeVault {
  files = new Map<string, TFile>();
  modified = new Map<string, string>();
  modifiedBinary = new Map<string, ArrayBuffer>();
  created = new Map<string, string>();
  createdBinary = new Map<string, ArrayBuffer>();
  folders = new Set<string>();

  constructor(paths: string[] = []) {
    for (const path of paths) this.files.set(path, tfile(path));
  }

  getAbstractFileByPath(path: string): TFile | null {
    return this.files.get(path) ?? null;
  }

  async createFolder(path: string): Promise<void> {
    this.folders.add(path);
  }

  async modify(file: TFile, content: string): Promise<void> {
    this.modified.set(file.path, content);
  }

  async create(path: string, content: string): Promise<TFile> {
    const file = tfile(path);
    this.files.set(path, file);
    this.created.set(path, content);
    return file;
  }

  async modifyBinary(file: TFile, bytes: ArrayBuffer): Promise<void> {
    this.modifiedBinary.set(file.path, bytes);
  }

  async createBinary(path: string, bytes: ArrayBuffer): Promise<TFile> {
    const file = tfile(path);
    this.files.set(path, file);
    this.createdBinary.set(path, bytes);
    return file;
  }
}

describe("restoreFileToCommit", () => {
  it("modifies existing text files with historical content", async () => {
    const vault = new FakeVault(["note.md"]);
    const api = {
      readFileAt: vi.fn().mockResolvedValue({ kind: "text", text: "old" })
    } as unknown as HistoryApi;

    await expect(
      restoreFileToCommit({
        vault: vault as any,
        api,
        vaultId: "v1",
        path: "note.md",
        atCommit: "c1",
        isBinary: false
      })
    ).resolves.toEqual({ ok: true, kind: "modified", bytes: 3 });
    expect(vault.modified.get("note.md")).toBe("old");
  });

  it("creates missing text files and parent folders", async () => {
    const vault = new FakeVault();
    const api = {
      readFileAt: vi.fn().mockResolvedValue({ kind: "text", text: "old" })
    } as unknown as HistoryApi;

    const result = await restoreFileToCommit({
      vault: vault as any,
      api,
      vaultId: "v1",
      path: "folder/note.md",
      atCommit: "c1",
      isBinary: false
    });

    expect(result).toEqual({ ok: true, kind: "created", bytes: 3 });
    expect(vault.folders.has("folder")).toBe(true);
    expect(vault.created.get("folder/note.md")).toBe("old");
  });

  it("writes binary versions with modifyBinary", async () => {
    const vault = new FakeVault(["image.png"]);
    const bytes = new Uint8Array([1, 2, 3]).buffer;
    const api = {
      readFileAt: vi.fn().mockResolvedValue({ kind: "binary", bytes })
    } as unknown as HistoryApi;

    const result = await restoreFileToCommit({
      vault: vault as any,
      api,
      vaultId: "v1",
      path: "image.png",
      atCommit: "c1",
      isBinary: true
    });

    expect(result).toEqual({ ok: true, kind: "modified", bytes: 3 });
    expect(vault.modifiedBinary.get("image.png")).toBe(bytes);
  });

  it("returns fetch_failed when historical content cannot be read", async () => {
    const vault = new FakeVault(["note.md"]);
    const api = {
      readFileAt: vi.fn().mockRejectedValue(new Error("missing"))
    } as unknown as HistoryApi;

    await expect(
      restoreFileToCommit({
        vault: vault as any,
        api,
        vaultId: "v1",
        path: "note.md",
        atCommit: "c1",
        isBinary: false
      })
    ).resolves.toMatchObject({ ok: false, reason: "fetch_failed" });
    expect(vault.modified.size).toBe(0);
  });
});
