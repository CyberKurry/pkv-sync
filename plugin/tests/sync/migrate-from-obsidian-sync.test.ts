import { TFile, TFolder } from "obsidian";
import { describe, expect, it, vi } from "vitest";
import {
  detectObsidianSync,
  migrateToPkv,
  scanVaultForMigration
} from "../../src/sync/migrate-from-obsidian-sync";
import type { PushChange, PushResponse, StateResponse } from "../../src/sync/types";
import type { VaultSummary } from "../../src/api/types";

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
  private readonly binary = new Map<string, ArrayBuffer>();

  addFile(path: string, size: number, content = ""): void {
    this.entries.set(path, tfile(path, size));
    this.text.set(path, content);
  }

  addBinaryFile(path: string, bytes: Uint8Array): void {
    const copy = new ArrayBuffer(bytes.byteLength);
    new Uint8Array(copy).set(bytes);
    this.entries.set(path, tfile(path, bytes.byteLength));
    this.binary.set(path, copy);
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

  async readBinary(file: TFile): Promise<ArrayBuffer> {
    const bytes = this.binary.get(file.path);
    if (bytes === undefined) throw new Error(`Unreadable binary file: ${file.path}`);
    return bytes.slice(0);
  }
}

class FakeMigrationApi {
  createdNames: string[] = [];
  pushes: Array<{
    vaultId: string;
    ifMatch: string | null;
    changes: PushChange[];
    deviceName: string;
  }> = [];
  uploadedBlobs: Array<{ vaultId: string; hash: string; bytes: ArrayBuffer }> = [];
  checkedHashes: string[][] = [];
  nextPushError: Error | null = null;
  private commitCounter = 0;

  async createVault(name: string): Promise<VaultSummary> {
    this.createdNames.push(name);
    return {
      id: "vault-1",
      user_id: "user-1",
      name,
      created_at: 1,
      last_sync_at: null,
      size_bytes: 0,
      file_count: 0
    };
  }

  async state(_vaultId: string, _headSince: string | null): Promise<StateResponse> {
    return { current_head: null, changed_since: false };
  }

  async uploadCheck(_vaultId: string, hashes: string[]): Promise<{ missing: string[] }> {
    this.checkedHashes.push(hashes);
    return { missing: hashes };
  }

  async uploadBlob(vaultId: string, hash: string, bytes: ArrayBuffer): Promise<void> {
    this.uploadedBlobs.push({ vaultId, hash, bytes });
  }

  async push(
    vaultId: string,
    ifMatch: string | null,
    changes: PushChange[],
    deviceName: string
  ): Promise<PushResponse> {
    this.pushes.push({ vaultId, ifMatch, changes, deviceName });
    if (this.nextPushError) throw this.nextPushError;
    this.commitCounter += 1;
    return { new_commit: `c${this.commitCounter}`, files_changed: changes.length };
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
    vault.addFile(".git/config", 9);
    vault.addFile("folder/write.lock", 9);
    vault.addFile("folder/upload.tmp", 10);
    vault.addFile(".DS_Store", 11);
    vault.addFile("Thumbs.db", 12);

    const result = scanVaultForMigration(vault);

    expect(result.files.map((file) => file.path)).toEqual([
      "note.md",
      "folder/image.png"
    ]);
    expect(result.skippedCount).toBe(13);
    expect(result.totalBytes).toBe(46);
  });
});

describe("migrateToPkv", () => {
  it("creates a new vault, scans eligible files, and reports skipped files", async () => {
    const vault = new FakeVault();
    vault.addFile("note.md", 5, "hello");
    vault.addFile(".obsidian/sync/state.json", 2, "{}");
    const api = new FakeMigrationApi();
    const onProgress = vi.fn();

    const result = await migrateToPkv({
      vault,
      api,
      vaultName: "Migrated vault",
      deviceName: "Laptop",
      textExtensions: new Set(["md"]),
      onProgress
    });

    expect(api.createdNames).toEqual(["Migrated vault"]);
    expect(result).toMatchObject({
      vaultId: "vault-1",
      vaultName: "Migrated vault",
      scannedFiles: 1,
      pushedFiles: 1,
      skippedCount: 1,
      batches: 1
    });
    expect(onProgress).toHaveBeenCalledWith(
      expect.objectContaining({
        stage: "scanning",
        totalFiles: 1,
        skippedCount: 1
      })
    );
  });

  it("pushes text files in batches with the latest commit as if-match", async () => {
    const vault = new FakeVault();
    vault.addFile("a.md", 1, "a");
    vault.addFile("b.md", 1, "b");
    vault.addFile("c.md", 1, "c");
    const api = new FakeMigrationApi();

    await migrateToPkv({
      vault,
      api,
      vaultName: "Batch vault",
      deviceName: "Laptop",
      textExtensions: new Set(["md"]),
      batchSize: 2
    });

    expect(api.pushes).toHaveLength(2);
    expect(api.pushes[0]).toMatchObject({
      vaultId: "vault-1",
      ifMatch: null,
      deviceName: "Laptop",
      changes: [
        { kind: "text", path: "a.md", content: "a" },
        { kind: "text", path: "b.md", content: "b" }
      ]
    });
    expect(api.pushes[1]).toMatchObject({
      vaultId: "vault-1",
      ifMatch: "c1",
      changes: [{ kind: "text", path: "c.md", content: "c" }]
    });
  });

  it("uploads binary blobs before pushing blob changes", async () => {
    const vault = new FakeVault();
    vault.addBinaryFile("image.png", new Uint8Array([1, 2, 3]));
    const api = new FakeMigrationApi();

    await migrateToPkv({
      vault,
      api,
      vaultName: "Blob vault",
      deviceName: "Laptop",
      textExtensions: new Set(["md"])
    });

    expect(api.checkedHashes).toEqual([[expect.any(String)]]);
    expect(api.uploadedBlobs).toHaveLength(1);
    expect(api.pushes[0].changes).toEqual([
      {
        kind: "blob",
        path: "image.png",
        blob_hash: api.uploadedBlobs[0].hash,
        size: 3,
        mime: "image/png"
      }
    ]);
  });

  it("emits progress updates while uploading blobs and pushing batches", async () => {
    const vault = new FakeVault();
    vault.addFile("a.md", 1, "a");
    vault.addBinaryFile("image.png", new Uint8Array([1]));
    const api = new FakeMigrationApi();
    const onProgress = vi.fn();

    await migrateToPkv({
      vault,
      api,
      vaultName: "Progress vault",
      deviceName: "Laptop",
      textExtensions: new Set(["md"]),
      batchSize: 1,
      onProgress
    });

    expect(onProgress).toHaveBeenCalledWith(
      expect.objectContaining({ stage: "uploading_blobs", uploadedBlobs: 1 })
    );
    expect(onProgress).toHaveBeenCalledWith(
      expect.objectContaining({
        stage: "pushing",
        processedFiles: 2,
        pushedFiles: 2,
        currentBatch: 2,
        totalBatches: 2
      })
    );
    expect(onProgress).toHaveBeenLastCalledWith(
      expect.objectContaining({ stage: "complete", pushedFiles: 2 })
    );
  });

  it("reports batch number and cause when a push batch fails", async () => {
    const vault = new FakeVault();
    vault.addFile("a.md", 1, "a");
    const api = new FakeMigrationApi();
    api.nextPushError = new Error("push failed");

    await expect(
      migrateToPkv({
        vault,
        api,
        vaultName: "Failure vault",
        deviceName: "Laptop",
        textExtensions: new Set(["md"])
      })
    ).rejects.toMatchObject({
      message: "Migration failed while pushing batch 1 of 1: push failed",
      batch: 1,
      totalBatches: 1,
      cause: api.nextPushError
    });
  });
});
