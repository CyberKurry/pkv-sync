import { Notice } from "obsidian";
import { ApiError } from "../api/client";
import { subscribeVaultEvents } from "../api/events-client";
import type { SyncApi } from "../api/sync-client";
import type { VaultEvent } from "../api/types";
import type { VaultSettings } from "../api/types";
import { format, strings, type Strings } from "../i18n";
import { createPathMatcher } from "./exclude";
import { conflictPath } from "./conflict";
import { sha256Bytes, sha256Text } from "./hash";
import { guessMime } from "./mime";
import {
  markDeleted,
  markSynced,
  pendingFiles
} from "./index-store";
import { textByteLength } from "./text-encoding";
import type { PushChange } from "./types";
import type { LocalFileSnapshot, LocalIndex, PullFile, PullResponse } from "./types";
import { debugLog, errorToMessage } from "../util";
import {
  shouldAcceptRemoteConflictPath,
  shouldSyncPath,
  type VaultAdapter
} from "./vault-adapter";

const BLOB_UPLOAD_CONCURRENCY = 4;

interface PendingScan {
  pending: LocalFileSnapshot[];
  deleted: string[];
  index: LocalIndex;
}

export interface IndexPersistence {
  loadIndex(): Promise<LocalIndex>;
  saveIndex(index: LocalIndex): Promise<void>;
  /**
   * Atomic read-modify-write of the sync index. Two concurrent calls are
   * serialised through the underlying plugin data store so neither caller
   * observes a stale read or overwrites the other's write. This is the
   * recommended path for any change that depends on the previous index
   * state (mark a file synced after inline apply, advance HEAD, etc.).
   */
  updateIndex(
    updater: (index: LocalIndex) => LocalIndex | Promise<LocalIndex>
  ): Promise<void>;
}

/**
 * Thrown by applyInlineText when the local file has diverged from what the
 * sync index recorded last. Signals the caller to fall back to a full pull
 * (which generates a .conflict-* file) instead of overwriting user edits.
 */
export class InlineApplyDirtyError extends Error {
  constructor(public readonly path: string) {
    super(`local copy of ${path} has unsynced changes; refusing inline apply`);
    this.name = "InlineApplyDirtyError";
  }
}

export interface SyncEngineOptions {
  vaultId: string;
  deviceName: string;
  textExtensions: Set<string>;
  extraExcludeGlobs?: string[];
  vault: VaultAdapter;
  api: SyncApi;
  index: IndexPersistence;
  deviceId?: string;
  serverUrl?: string;
  deploymentKey?: string;
  token?: string;
  pluginVersion?: string;
  setStatus(
    status: "connected" | "syncing" | "offline" | "error",
    detail?: string
  ): void;
  labels?: Pick<Strings, "inlineApplyFailed">;
  onSyncSuccess?(): void | Promise<void>;
  vaultSettingsReader?: (vaultId: string) => Promise<VaultSettings>;
}

export class SyncEngine {
  private running: Promise<void> | null = null;
  private unsubscribeEvents: (() => void) | null = null;
  private vaultSettingsCache = new Map<string, VaultSettings>();
  /**
   * Chain head for serialising SSE event handlers. Each incoming event
   * appends a task that awaits the previous one before running, so events
   * never interleave between each other or with syncNow's atomic phase
   */
  private eventChain: Promise<void> = Promise.resolve();

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
    let timeoutId: number | undefined;
    try {
      await Promise.race([
        this.syncNow(),
        new Promise<void>((resolve) => {
          timeoutId = window.setTimeout(resolve, timeoutMs);
        })
      ]);
    } finally {
      window.clearTimeout(timeoutId);
    }
  }

  startEventSubscription(): void {
    if (!this.opts.serverUrl || !this.opts.deviceId || !this.opts.deploymentKey || !this.opts.token) return;
    this.unsubscribeEvents?.();
    this.unsubscribeEvents = subscribeVaultEvents({
      serverUrl: this.opts.serverUrl,
      vaultId: this.opts.vaultId,
      deploymentKey: this.opts.deploymentKey,
      token: this.opts.token,
      ownDeviceId: this.opts.deviceId,
      pluginVersion: this.opts.pluginVersion ?? "0.0.0",
      onEvent: (ev: VaultEvent) => {
        // Serialise all SSE event handling through eventChain so that two
        // quick-succession events cannot interleave their applyInlineText /
        // applyDelete / advanceIndexHead steps. Without this, event B could
        // read a stale local file between event A's writeText and A's
        // index update, leading to either lost content or a same-commit
        // echo push back to the server.
        const task = this.eventChain.then(async () => {
          if (!ev.commit) {
            // lagged — do a full pull
            await this.syncNow();
            return;
          }
          if (ev.kind === "rollback") {
            await this.syncNow();
            return;
          }
          let needFallbackPull = false;
          for (const change of ev.changes) {
            try {
              switch (change.kind) {
                case "text_inline":
                  await this.applyInlineText(change.path, change.content, ev.commit);
                  break;
                case "delete":
                  await this.applyDelete(change.path, ev.commit);
                  break;
                case "text_ref":
                case "blob":
                  needFallbackPull = true;
                  break;
              }
            } catch (err) {
              if (
                change.kind === "text_inline" &&
                !(err instanceof InlineApplyDirtyError)
              ) {
                this.reportInlineApplyFailure(change.path, err);
              } else if (!(err instanceof InlineApplyDirtyError)) {
                debugLog("[pkv-sync] inline apply failed, falling back to pull:", err);
              }
              needFallbackPull = true;
            }
          }
          if (needFallbackPull) {
            await this.syncNow();
          } else {
            await this.advanceIndexHead(ev.commit);
          }
        });
        this.eventChain = task.catch(() => undefined);
      },
      onError: (err: Error) => {
        debugLog("[pkv-sync] SSE event stream error; automatic reconnect will continue:", err);
      },
    });
  }

  stopEventSubscription(): void {
    this.unsubscribeEvents?.();
    this.unsubscribeEvents = null;
  }

  async scanPending(): Promise<{
    pending: LocalFileSnapshot[];
    deleted: string[];
    index: LocalIndex;
  }> {
    const index = await this.opts.index.loadIndex();
    const current = await this.opts.vault.scan(this.opts.textExtensions, index);
    return this.scanPendingFrom(index, current);
  }

  private scanPendingFrom(
    index: LocalIndex,
    current: LocalFileSnapshot[]
  ): PendingScan {
    const pathAccepted = this.currentPathMatcher();
    const filtered = current.filter((f) => pathAccepted(f.path));
    const currentPaths = new Set(filtered.map((f) => f.path));
    const deletedFromIndex = Object.keys(index.files).filter((p) => !currentPaths.has(p));
    return {
      pending: pendingFiles(index, filtered),
      deleted: deletedFromIndex.filter((p) => pathAccepted(p)),
      index
    };
  }

  private currentPathMatcher(): (path: string) => boolean {
    const userExcludes = this.opts.extraExcludeGlobs ?? [];
    const userAllowlist =
      this.vaultSettingsCache.get(this.opts.vaultId)?.extra_sync_globs ?? [];
    return createPathMatcher({ userExcludes, userAllowlist });
  }

  private async syncInner(): Promise<void> {
    this.opts.setStatus("syncing");
    try {
      await this.refreshVaultSettings();
      const scan = await this.pullIfChanged();
      await this.pushPendingWithHeadMismatchRetry(scan);
      this.opts.setStatus("connected");
      await this.opts.onSyncSuccess?.();
    } catch (error) {
      if (error instanceof ApiError && error.status === 0) {
        this.opts.setStatus("offline", error.message);
      } else {
        this.opts.setStatus(
          "error",
          errorToMessage(error)
        );
      }
      throw error;
    }
  }

  private async refreshVaultSettings(): Promise<void> {
    const reader = this.vaultSettingsReader();
    if (!reader) return;
    try {
      const settings = await reader(this.opts.vaultId);
      this.vaultSettingsCache.set(this.opts.vaultId, {
        extra_sync_globs: Array.isArray(settings.extra_sync_globs)
          ? settings.extra_sync_globs.filter((glob) => typeof glob === "string")
          : []
      });
    } catch (error) {
      debugLog("[pkv-sync] failed to refresh vault settings; using cached settings:", error);
    }
  }

  private vaultSettingsReader():
    | ((vaultId: string) => Promise<VaultSettings>)
    | null {
    return (
      this.opts.vaultSettingsReader ??
      this.opts.api.getVaultSettings?.bind(this.opts.api) ??
      null
    );
  }

  private async pullIfChanged(): Promise<PendingScan | null> {
    const index = await this.opts.index.loadIndex();
    const pull = await this.opts.api.pull(
      this.opts.vaultId,
      index.lastSyncedCommit
    );
    return this.applyPull(pull);
  }

  private async pushPending(scan?: PendingScan | null): Promise<void> {
    const { pending, deleted, index } = scan ?? await this.scanPending();
    if (pending.length === 0 && deleted.length === 0) return;

    const blobFiles = pending.filter((file) => file.kind === "blob");
    const blobHashes = blobFiles.map((file) => file.hash);
    const missing =
      blobHashes.length > 0
        ? (await this.opts.api.uploadCheck(this.opts.vaultId, blobHashes)).missing
        : [];
    const missingSet = new Set(missing);
    await uploadMissingBlobs(
      this.opts.api,
      this.opts.vaultId,
      blobFiles.filter((file) => missingSet.has(file.hash))
    );

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

  private async pushPendingWithHeadMismatchRetry(
    scan: PendingScan | null
  ): Promise<void> {
    try {
      await this.pushPending(scan);
    } catch (error) {
      if (
        error instanceof ApiError &&
        error.status === 409 &&
        error.code === "head_mismatch"
      ) {
        const retryScan = await this.pullIfChanged();
        await this.pushPending(retryScan);
        return;
      }
      throw error;
    }
  }

  private async applyPull(pull: PullResponse): Promise<PendingScan | null> {
    if (!pull.to) return null;
    let index = await this.opts.index.loadIndex();
    const current = await this.opts.vault.scan(this.opts.textExtensions, index);
    const currentByPath = new Map(current.map((file) => [file.path, file]));
    const nextCurrentByPath = new Map(currentByPath);
    const pathAccepted = this.currentPathMatcher();
    const pulledText = new Map<string, string>();
    const touched: LocalFileSnapshot[] = [];
    const deleted: string[] = [];

    try {
      for (const file of [...pull.added, ...pull.modified]) {
        if (!this.shouldApplyPulledPath(file.path, pathAccepted)) continue;
        const local = currentByPath.get(file.path);
        const indexed = index.files[file.path];
        if (isLocalDeleted(local, indexed?.lastSyncedHash)) {
          await this.writeRemoteConflict(file, pull.to);
          continue;
        }

        if (file.file_type === "text") {
          const content = await this.pulledTextContent(file, pull.to, pulledText);
          const hash = await sha256Text(content);
          if (local?.kind === "text" && local.hash === hash) {
            touched.push(local);
            continue;
          }
          if (isLocalDirty(local, indexed?.lastSyncedHash)) {
            await this.writeConflict(file.path, local);
          }
          await this.opts.vault.writeText(file.path, content);
          touched.push({
            path: file.path,
            hash,
            size: textByteLength(content),
            kind: "text",
            content
          });
        } else {
          if (!file.blob_hash) throw new Error(`Missing blob hash for ${file.path}`);
          if (local?.kind === "blob" && local.hash === file.blob_hash) {
            touched.push(local);
            continue;
          }
          if (isLocalDirty(local, indexed?.lastSyncedHash)) {
            await this.writeConflict(file.path, local);
          }
          const bytes = await this.opts.api.downloadBlob(
            this.opts.vaultId,
            file.blob_hash
          );
          const actualHash = await sha256Bytes(bytes);
          if (actualHash !== file.blob_hash) {
            throw new Error(`Blob hash mismatch for ${file.path}`);
          }
          await this.opts.vault.writeBinary(file.path, bytes);
          touched.push({
            path: file.path,
            hash: actualHash,
            size: file.size,
            kind: "blob",
            bytes
          });
        }
      }

      for (const path of pull.deleted) {
        if (!shouldSyncPath(path) || !pathAccepted(path)) continue;
        const local = currentByPath.get(path);
        const indexed = index.files[path];
        if (isLocalDirty(local, indexed?.lastSyncedHash)) {
          await this.writeConflict(path, local);
        }
        await this.opts.vault.delete(path);
        deleted.push(path);
      }
    } catch (error) {
      await this.savePartialPullProgress(index, touched, deleted);
      throw error;
    }

    index = markSynced(
      index,
      pull.to,
      touched.filter((file) => shouldSyncPath(file.path))
    );
    index = markDeleted(
      index,
      pull.to,
      pull.deleted.filter((path) => shouldSyncPath(path) && pathAccepted(path))
    );
    await this.opts.index.saveIndex(index);
    for (const file of touched) {
      if (shouldSyncPath(file.path)) {
        nextCurrentByPath.set(file.path, file);
      }
    }
    for (const path of deleted) {
      nextCurrentByPath.delete(path);
    }
    return this.scanPendingFrom(index, [...nextCurrentByPath.values()]);
  }

  private async pulledTextContent(
    file: PullFile,
    atCommit: string,
    cache: Map<string, string>
  ): Promise<string> {
    if (file.file_type !== "text") {
      throw new Error(`Cannot read text content for blob ${file.path}`);
    }
    if (file.content_inline !== null && file.content_inline !== undefined) {
      return file.content_inline;
    }
    const cached = cache.get(file.path);
    if (cached !== undefined) return cached;
    const content = await this.opts.api.downloadTextFile(
      this.opts.vaultId,
      file.path,
      atCommit
    );
    cache.set(file.path, content);
    return content;
  }

  private shouldApplyPulledPath(
    path: string,
    pathAccepted: (path: string) => boolean
  ): boolean {
    return (
      (shouldSyncPath(path) && pathAccepted(path)) ||
      shouldAcceptRemoteConflictPath(path)
    );
  }

  private async savePartialPullProgress(
    index: LocalIndex,
    touched: LocalFileSnapshot[],
    deleted: string[]
  ): Promise<void> {
    if (touched.length === 0 && deleted.length === 0) return;
    let partial = markSynced(index, index.lastSyncedCommit, touched);
    partial = markDeleted(partial, index.lastSyncedCommit, deleted);
    await this.opts.index.saveIndex(partial);
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

  private async applyInlineText(path: string, content: string, commit: string): Promise<void> {
    // Dirty check + write must look at one consistent index snapshot.
    // updateIndex serialises through the underlying data store so two
    // concurrent inline-event handlers cannot observe stale state or
    // overwrite each other's index updates.
    const hash = await sha256Text(content);
    const size = textByteLength(content);
    const snapshot: LocalFileSnapshot = { path, hash, size, kind: "text", content };
    await this.opts.index.updateIndex(async (index) => {
      const indexed = index.files[path];
      if (indexed && this.opts.vault.exists(path)) {
        const localContent = await this.opts.vault.readText(path);
        const localHash = await sha256Text(localContent);
        if (localHash !== indexed.lastSyncedHash) {
          // Local has unsynced edits. Throwing inside updateIndex aborts
          // the data-store update without writing, so the index does not
          // advance and the file is not touched.
          throw new InlineApplyDirtyError(path);
        }
      }
      // Write file inside the atomic update so a concurrent applyInlineText
      // cannot interleave its writeText between our dirty check and our save.
      await this.opts.vault.writeText(path, content);
      return markSynced(index, commit, [snapshot]);
    });
  }

  private reportInlineApplyFailure(path: string, error: unknown): void {
    const message = format(
      (this.opts.labels ?? strings()).inlineApplyFailed,
      {
        path,
        reason: errorToMessage(error)
      }
    );
    debugLog("[pkv-sync] inline apply failed, falling back to pull:", error);
    new Notice(message);
    this.opts.setStatus("error", message);
  }

  private async applyDelete(path: string, commit: string): Promise<void> {
    await this.opts.index.updateIndex(async (index) => {
      await this.opts.vault.delete(path);
      return markDeleted(index, commit, [path]);
    });
  }

  private async advanceIndexHead(commit: string): Promise<void> {
    await this.opts.index.updateIndex(async (index) => {
      if (index.lastSyncedCommit === commit) return index;
      return { ...index, lastSyncedCommit: commit };
    });
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

async function uploadMissingBlobs(
  api: SyncApi,
  vaultId: string,
  files: LocalFileSnapshot[]
): Promise<void> {
  for (const file of files) {
    if (!file.bytes) throw new Error(`Missing bytes for blob ${file.path}`);
  }

  let next = 0;
  const workerCount = Math.min(BLOB_UPLOAD_CONCURRENCY, files.length);
  const workers = Array.from({ length: workerCount }, async () => {
    while (next < files.length) {
      const file = files[next];
      next += 1;
      if (!file.bytes) throw new Error(`Missing bytes for blob ${file.path}`);
      await api.uploadBlob(vaultId, file.hash, file.bytes);
    }
  });
  await Promise.all(workers);
}
