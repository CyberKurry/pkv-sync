import { Notice } from "obsidian";
import { ApiError } from "../api/client";
import type { SyncApi } from "../api/sync-client";
import { conflictPath } from "./conflict";
import { sha256Text } from "./hash";
import {
  deletedFiles,
  markDeleted,
  markSynced,
  pendingFiles
} from "./index-store";
import type { PushChange } from "./types";
import type { LocalFileSnapshot, LocalIndex, PullFile, PullResponse } from "./types";
import { shouldSyncPath, type VaultAdapter } from "./vault-adapter";

export interface IndexPersistence {
  loadIndex(): Promise<LocalIndex>;
  saveIndex(index: LocalIndex): Promise<void>;
}

export interface SyncEngineOptions {
  vaultId: string;
  deviceName: string;
  textExtensions: Set<string>;
  vault: VaultAdapter;
  api: SyncApi;
  index: IndexPersistence;
  setStatus(
    status: "connected" | "syncing" | "offline" | "error",
    detail?: string
  ): void;
}

export class SyncEngine {
  private running: Promise<void> | null = null;

  constructor(private opts: SyncEngineOptions) {
    if (!opts.vaultId.trim()) throw new Error("SyncEngine requires a non-empty vaultId");
    if (!opts.deviceName.trim()) {
      throw new Error("SyncEngine requires a non-empty deviceName");
    }
  }

  async syncNow(): Promise<void> {
    if (this.running) return this.running;
    this.running = this.syncInner().finally(() => {
      this.running = null;
    });
    return this.running;
  }

  async flushOnUnload(timeoutMs: number): Promise<void> {
    await Promise.race([
      this.syncNow(),
      new Promise<void>((resolve) => window.setTimeout(resolve, timeoutMs))
    ]);
  }

  async scanPending(): Promise<{
    pending: LocalFileSnapshot[];
    deleted: string[];
    index: LocalIndex;
  }> {
    const index = await this.opts.index.loadIndex();
    const current = await this.opts.vault.scan(this.opts.textExtensions);
    return {
      pending: pendingFiles(index, current),
      deleted: deletedFiles(index, current),
      index
    };
  }

  private async syncInner(): Promise<void> {
    this.opts.setStatus("syncing");
    try {
      await this.pullIfChanged();
      await this.pushPendingWithHeadMismatchRetry();
      this.opts.setStatus("connected");
    } catch (error) {
      if (error instanceof ApiError && error.status === 0) {
        this.opts.setStatus("offline", error.message);
      } else {
        this.opts.setStatus(
          "error",
          error instanceof Error ? error.message : String(error)
        );
      }
      throw error;
    }
  }

  private async pullIfChanged(): Promise<void> {
    const index = await this.opts.index.loadIndex();
    const state = await this.opts.api.state(
      this.opts.vaultId,
      index.lastSyncedCommit
    );
    if (!state.changed_since) return;
    const pull = await this.opts.api.pull(
      this.opts.vaultId,
      index.lastSyncedCommit
    );
    await this.applyPull(pull);
  }

  private async pushPending(): Promise<void> {
    const { pending, deleted, index } = await this.scanPending();
    if (pending.length === 0 && deleted.length === 0) return;

    const blobFiles = pending.filter((file) => file.kind === "blob");
    const blobHashes = blobFiles.map((file) => file.hash);
    const missing =
      blobHashes.length > 0
        ? (await this.opts.api.uploadCheck(this.opts.vaultId, blobHashes)).missing
        : [];
    const missingSet = new Set(missing);
    for (const file of blobFiles) {
      if (!missingSet.has(file.hash)) continue;
      if (!file.bytes) throw new Error(`Missing bytes for blob ${file.path}`);
      await this.opts.api.uploadBlob(this.opts.vaultId, file.hash, file.bytes);
    }

    const changes: PushChange[] = [
      ...pending.map((file) => {
        if (file.kind === "text") {
          return {
            kind: "text" as const,
            path: file.path,
            content: file.content ?? ""
          };
        }
        return {
          kind: "blob" as const,
          path: file.path,
          blob_hash: file.hash,
          size: file.size,
          mime: guessMime(file.path)
        };
      }),
      ...deleted.map((path) => ({ kind: "delete" as const, path }))
    ];
    if (changes.length > 1000) {
      throw new Error(
        "Too many pending changes for one sync pass; run manual sync after reducing batch size"
      );
    }

    const response = await this.opts.api.push(
      this.opts.vaultId,
      index.lastSyncedCommit,
      changes,
      this.opts.deviceName
    );
    let next = markSynced(index, response.new_commit, pending);
    next = markDeleted(next, response.new_commit, deleted);
    await this.opts.index.saveIndex(next);
  }

  private async pushPendingWithHeadMismatchRetry(): Promise<void> {
    try {
      await this.pushPending();
    } catch (error) {
      if (
        error instanceof ApiError &&
        error.status === 409 &&
        error.code === "head_mismatch"
      ) {
        await this.pullIfChanged();
        await this.pushPending();
        return;
      }
      throw error;
    }
  }

  private async applyPull(pull: PullResponse): Promise<void> {
    if (!pull.to) return;
    let index = await this.opts.index.loadIndex();
    const current = await this.opts.vault.scan(this.opts.textExtensions);
    const currentByPath = new Map(current.map((file) => [file.path, file]));
    const touched: LocalFileSnapshot[] = [];

    for (const file of [...pull.added, ...pull.modified]) {
      if (!shouldSyncPath(file.path)) continue;
      const local = currentByPath.get(file.path);
      const indexed = index.files[file.path];
      if (isLocalDeleted(local, indexed?.lastSyncedHash)) {
        await this.writeRemoteConflict(file, pull.to);
        continue;
      }
      if (isLocalDirty(local, indexed?.lastSyncedHash)) {
        await this.writeConflict(file.path, local);
      }

      if (file.file_type === "text") {
        const content =
          file.content_inline ??
          (await this.opts.api.downloadTextFile(
            this.opts.vaultId,
            file.path,
            pull.to
          ));
        await this.opts.vault.writeText(file.path, content);
        touched.push({
          path: file.path,
          hash: await sha256Text(content),
          size: new TextEncoder().encode(content).byteLength,
          kind: "text",
          content
        });
      } else {
        if (!file.blob_hash) throw new Error(`Missing blob hash for ${file.path}`);
        const bytes = await this.opts.api.downloadBlob(
          this.opts.vaultId,
          file.blob_hash
        );
        await this.opts.vault.writeBinary(file.path, bytes);
        touched.push({
          path: file.path,
          hash: file.blob_hash,
          size: file.size,
          kind: "blob",
          bytes
        });
      }
    }

    for (const path of pull.deleted) {
      if (!shouldSyncPath(path)) continue;
      const local = currentByPath.get(path);
      const indexed = index.files[path];
      if (isLocalDirty(local, indexed?.lastSyncedHash)) {
        await this.writeConflict(path, local);
      }
      await this.opts.vault.delete(path);
    }

    index = markSynced(index, pull.to, touched);
    index = markDeleted(index, pull.to, pull.deleted);
    await this.opts.index.saveIndex(index);
  }

  private async writeConflict(
    path: string,
    local: LocalFileSnapshot | undefined
  ): Promise<void> {
    if (!local) return;
    const cpath = conflictPath(path, this.opts.deviceName);
    if (local.kind === "text") {
      await this.opts.vault.writeText(cpath, local.content ?? "");
    } else if (local.bytes) {
      await this.opts.vault.writeBinary(cpath, local.bytes);
    }
    new Notice(`PKV Sync conflict: ${cpath}`);
  }

  private async writeRemoteConflict(file: PullFile, atCommit: string): Promise<void> {
    const cpath = conflictPath(file.path, "remote");
    if (file.file_type === "text") {
      const content =
        file.content_inline ??
        (await this.opts.api.downloadTextFile(
          this.opts.vaultId,
          file.path,
          atCommit
        ));
      await this.opts.vault.writeText(cpath, content);
    } else {
      if (!file.blob_hash) throw new Error(`Missing blob hash for ${file.path}`);
      const bytes = await this.opts.api.downloadBlob(
        this.opts.vaultId,
        file.blob_hash
      );
      await this.opts.vault.writeBinary(cpath, bytes);
    }
    new Notice(`PKV Sync conflict: ${cpath}`);
  }
}

function isLocalDeleted(
  local: LocalFileSnapshot | undefined,
  lastSyncedHash: string | undefined
): boolean {
  return !local && !!lastSyncedHash;
}

function isLocalDirty(
  local: LocalFileSnapshot | undefined,
  lastSyncedHash: string | undefined
): local is LocalFileSnapshot {
  if (!local) return false;
  return !lastSyncedHash || local.hash !== lastSyncedHash;
}

function guessMime(path: string): string | undefined {
  const ext = path.split(".").pop()?.toLowerCase();
  if (!ext) return undefined;
  const map: Record<string, string> = {
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    gif: "image/gif",
    webp: "image/webp",
    pdf: "application/pdf",
    mp3: "audio/mpeg",
    wav: "audio/wav",
    mp4: "video/mp4"
  };
  return map[ext];
}
