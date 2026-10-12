import { Notice } from "obsidian";
import { ApiError } from "../api/client";
import { subscribeVaultEvents } from "../api/events-client";
import type { SyncApi } from "../api/sync-client";
import type { VaultEvent } from "../api/types";
import type { VaultSettings } from "../api/types";
import { format, strings, type Strings } from "../i18n";
import { createPathMatcher } from "./exclude";
import { conflictPath } from "./conflict";
import { sha256Bytes, sha256Text, sha256TextWithLength } from "./hash";
import { guessMime } from "./mime";
import {
  markBatch,
  markDeleted,
  markFilesDeleted,
  markFilesSynced,
  markSynced
} from "./index-store";
import type { PushChange } from "./types";
import type { LocalFileSnapshot, LocalIndex, PullFile, PullResponse } from "./types";
import { debugLog, errorToMessage } from "../util";
import {
  shouldAcceptRemoteConflictPath,
  shouldSyncPath,
  type VaultAdapter
} from "./vault-adapter";

const BLOB_UPLOAD_CONCURRENCY = 4;
const MAX_PUSH_CHANGES = 1000;

interface PendingScan {
  pending: LocalFileSnapshot[];
  deleted: string[];
  index: LocalIndex;
}

export interface IndexPersistence {
  loadIndex(): Promise<LocalIndex>;
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

/**
 * Expected cancellation of an in-flight sync pass. The orchestrator aborts a
 * generation's engine on rebuild()/invalidate() (audit M-10); the engine
 * checks the signal before every vault write/trash and unwinds with this
 * error, which syncInner swallows silently because it is not a failure —
 * the replacement engine owns the vault from that point on.
 */
export class SyncAbortedError extends Error {
  constructor() {
    super("sync pass aborted: engine generation was replaced");
    this.name = "SyncAbortedError";
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
  private unsubscribeEvents: (() => void) | null = null;
  private vaultSettingsCache = new Map<string, VaultSettings>();
  /**
   * Aborted by the orchestrator when this generation is discarded
   * (rebuild()/invalidate(), audit M-10). Checked before every vault
   * write/trash and at each syncInner phase boundary so an in-flight pass
   * against the OLD vault/server cannot mutate the CURRENT local vault.
   */
  private abortController = new AbortController();
  /**
   * Chain head for serialising SSE event handlers AND syncNow's atomic
   * phase through one mutex (audit M-12). Each incoming event appends a
   * task that awaits the previous one before running, and each external
   * syncNow pass appends itself the same way — so events never interleave
   * between each other or with a sync pass's scan/push/pull/apply steps.
   * Without the shared chain, an event could apply newer content while an
   * in-flight pull was parked mid-apply; releasing the pull then wrote its
   * stale diff back over the newer content and regressed the head past
   * content that had already been delivered.
   */
  private eventChain: Promise<void> = Promise.resolve();
  /**
   * The in-flight (or queued) syncNow pass. Present while a pass is on the
   * chain so additional syncNow calls coalesce onto it instead of queueing
   * another pass behind it.
   */
  private activeSync: Promise<void> | null = null;

  constructor(private opts: SyncEngineOptions) {
    if (!opts.vaultId.trim()) throw new Error("SyncEngine requires a non-empty vaultId");
    if (!opts.deviceName.trim()) {
      throw new Error("SyncEngine requires a non-empty deviceName");
    }
  }

  /** Cancel any in-flight sync pass and future event handling (M-10). */
  abort(): void {
    this.abortController.abort();
  }

  private get aborted(): boolean {
    return this.abortController.signal.aborted;
  }

  /** Throws the typed cancellation when this generation was aborted. */
  private throwIfAborted(): void {
    if (this.aborted) throw new SyncAbortedError();
  }

  async syncNow(): Promise<void> {
    if (this.activeSync) return this.activeSync;
    // M-12: the pass runs as a chain task, so it both (a) waits for any
    // event handler that is mid-apply — the chain-runner equivalent of
    // "chain busy": the pass cannot start until the running task's
    // write/advanceIndexHead finish — and (b) holds the chain while it
    // runs, making events delivered mid-pass queue behind it. A bare
    // off-chain run (the old engine-global "nested" bypass) reopened the
    // pass/event interleave the chain exists to prevent: an external
    // syncNow (poll timer, rebuild kick) arriving while an event handler
    // was mid-apply, with no pass in flight to coalesce onto, ran
    // syncInner directly OFF the chain, concurrently with that event.
    const run = this.runInsideChain(() => this.syncInner());
    const completion = run.then(
      () => {
        if (this.activeSync === completion) this.activeSync = null;
      },
      (error) => {
        if (this.activeSync === completion) this.activeSync = null;
        throw error;
      }
    );
    this.activeSync = completion;
    return completion;
  }

  /**
   * syncNow for the SSE fallback paths, which already run INSIDE a chain
   * task. The chain is held by the caller's own task, so queueing would
   * wait for the very task that queued it — a self-deadlock (and so
   * would coalescing onto an external pass that is itself queued behind
   * this task). The pass instead runs inline within that task's slot:
   * serialization is preserved because nothing else starts on the chain
   * until the task — now including this pass — finishes. Nested-ness is
   * declared at the call site, never inferred from engine-global state:
   * an external syncNow must always queue, which is exactly the M-12
   * bypass-window fix.
   */
  private syncNowInsideChainTask(): Promise<void> {
    return this.syncInner();
  }

  /**
   * Run `task` as a chain task: ordered after everything already on the
   * chain, so it never starts while another task (an SSE event handler
   * or a sync pass) is mid-execution (M-12).
   */
  private runInsideChain(task: () => Promise<void>): Promise<void> {
    const run = this.eventChain.then(async () => {
      await task();
    });
    // Keep the chain clean even when the task fails.
    this.eventChain = run.then(() => undefined, () => undefined);
    return run;
  }

  /**
   * True once this generation's pass has been cancelled (M-10). Exposed so
   * callers that captured the engine before a rebuild/invalidate can stop
   * chaining follow-up work onto a dead generation.
   */
  isAborted(): boolean {
    return this.aborted;
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
        void this.runInsideChain(async () => {
          // A replaced/aborted generation must not apply events into the
          // vault either (M-10) — drop the event entirely; the replacement
          // engine's own sync covers the missed commits.
          if (this.aborted) return;
          if (!ev.commit) {
            // lagged — do a full pull
            await this.syncNowInsideChainTask();
            return;
          }
          if (ev.kind === "rollback") {
            await this.syncNowInsideChainTask();
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
            await this.syncNowInsideChainTask();
          } else {
            await this.advanceIndexHead(ev.commit);
          }
        }).catch(() => undefined);
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
    const current = await this.opts.vault.scan(this.opts.textExtensions, index, {
      retainPayload: false
    });
    return this.scanPendingFrom(index, current);
  }

  private scanPendingFrom(
    index: LocalIndex,
    current: LocalFileSnapshot[]
  ): PendingScan {
    const pathAccepted = this.currentPathMatcher();
    const currentPaths = new Set<string>();
    const pending: LocalFileSnapshot[] = [];
    for (const file of current) {
      if (!pathAccepted(file.path)) continue;
      currentPaths.add(file.path);
      if (index.files[file.path]?.lastSyncedHash !== file.hash) {
        pending.push(file);
      }
    }
    const deleted = Object.keys(index.files).filter(
      (p) => !currentPaths.has(p) && pathAccepted(p)
    );
    return { pending, deleted, index };
  }

  private matcherCache = new Map<string, (path: string) => boolean>();

  private currentPathMatcher(): (path: string) => boolean {
    const userExcludes = this.opts.extraExcludeGlobs ?? [];
    const userAllowlist =
      this.vaultSettingsCache.get(this.opts.vaultId)?.extra_sync_globs ?? [];
    // Compile the glob matcher once per settings value (exact key, no TTL) so
    // the same exclude/allowlist sets are not recompiled on every sync pass.
    const key = JSON.stringify([userExcludes, userAllowlist]);
    let matcher = this.matcherCache.get(key);
    if (!matcher) {
      matcher = createPathMatcher({ userExcludes, userAllowlist });
      this.matcherCache.set(key, matcher);
    }
    return matcher;
  }

  private async syncInner(): Promise<void> {
    try {
      this.throwIfAborted();
      this.opts.setStatus("syncing");
      await this.refreshVaultSettings();

      // ① scan BEFORE any pull
      const scan = await this.scanPending();
      this.throwIfAborted();

      // ② push-first: send pending changes before pulling
      await this.pushPendingWithHeadMismatchRetry(scan);
      this.throwIfAborted();

      // ③ pull backflow: merged content arrives as a normal pull
      await this.pullIfChanged();

      this.throwIfAborted();
      this.opts.setStatus("connected");
      await this.opts.onSyncSuccess?.();
    } catch (error) {
      if (error instanceof SyncAbortedError) {
        // Expected cancellation (generation replaced). No status change:
        // the replacement engine owns the status element now.
        return;
      }
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

    type PendingPushItem =
      | { kind: "file"; file: LocalFileSnapshot }
      | { kind: "delete"; path: string };
    const items: PendingPushItem[] = [
      ...pending.map((file) => ({ kind: "file" as const, file })),
      ...deleted.map((path) => ({ kind: "delete" as const, path }))
    ];
    let ifMatch = index.lastSyncedCommit;

    for (let offset = 0; offset < items.length; offset += MAX_PUSH_CHANGES) {
      const batch = items.slice(offset, offset + MAX_PUSH_CHANGES);
      const batchPending = batch
        .filter(
          (item): item is { kind: "file"; file: LocalFileSnapshot } =>
            item.kind === "file"
        )
        .map((item) => item.file);
      const batchDeleted = batch
        .filter(
          (item): item is { kind: "delete"; path: string } =>
            item.kind === "delete"
        )
        .map((item) => item.path);
      const hydrated = await this.hydratePendingFiles(batchPending);

      const blobFiles = hydrated.filter((file) => file.kind === "blob");
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
        ...hydrated.map((file) => {
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
        ...batchDeleted.map((path) => ({ kind: "delete" as const, path }))
      ];
      const response = await this.opts.api.push(
        this.opts.vaultId,
        ifMatch,
        changes,
        this.opts.deviceName
      );

    // `merge_outcomes` is present ONLY when the server took the auto-merge
    // path, which by definition means the server head had already moved past
    // our base — commits we never pulled live in (base, new_commit]. The
    // per-path outcomes only describe the paths WE pushed; "clean" says
    // nothing about those missed commits. So the mere presence of the field
    // means the head must stay at the old base and the pull in ③ must cover
    // the merge range (audit H-1: advancing on all-clean left remote changes
    // permanently missed, since pull(since=new_commit) returns empty).
      const merged = response.merge_outcomes !== undefined;

      if (!merged) {
      // Fast path (no auto-merge) — advance head immediately
        await this.opts.index.updateIndex((current) => {
          return markBatch(current, response.new_commit, hydrated, batchDeleted, true);
        });
        ifMatch = response.new_commit;
      } else {
      // Auto-merge (any outcome mix) — per-file hashes recorded, HEAD STAYS
      // at the old base. The subsequent pull will bring both the merged
      // content and the missed remote commits, then advance head. Use
      // updateIndex to avoid regressing lastSyncedCommit if a concurrent
      // SSE event advanced it between our scan and this write.
      //
      // Deletes are partitioned by per-path outcome (audit M-11): a delete
      // the server applied cleanly removes the index entry, but a REJECTED
      // delete (outcome "conflict" — the remote had modified the file)
      // keeps its original entry. Dropping it would make the subsequent
      // pull treat the path as brand new and silently write the remote
      // content back, undoing the user's delete; the surviving entry keeps
      // the path "locally deleted" from the pull's point of view, so the
      // remote content arrives as a .conflict-* copy instead of a silent
      // resurrection. Upserted files keep recording their pushed hash for
      // every outcome: the pushed content has been handed to the server
      // (merged into the result or parked in a server sidecar), so the
      // pull must apply the authoritative result without a spurious
      // conflict copy of content the server already holds.
        const cleanPaths = new Set(
          response.merge_outcomes
            ?.filter((o) => o.outcome === "clean")
            .map((o) => o.path) ?? []
        );
        const cleanDeleted = batchDeleted.filter((path) => cleanPaths.has(path));
        await this.opts.index.updateIndex((current) => {
          return markBatch(current, current.lastSyncedCommit, hydrated, cleanDeleted, false);
        });
      }
    }
  }

  private async hydratePendingFiles(
    files: LocalFileSnapshot[]
  ): Promise<LocalFileSnapshot[]> {
    return Promise.all(
      files.map(async (file) => {
        const hasPayload =
          file.kind === "text"
            ? file.content !== undefined
            : file.bytes !== undefined;
        return hasPayload
          ? file
          : this.opts.vault.snapshot(file.path, this.opts.textExtensions);
      })
    );
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
    if (pull.added.length === 0 && pull.modified.length === 0 && pull.deleted.length === 0) {
      // An abort that lands during the pull request must still stop the
      // head advance (M-10): the replacement engine owns the index.
      this.throwIfAborted();
      await this.advanceIndexHead(pull.to);
      return null;
    }
    let index = await this.opts.index.loadIndex();
    const current = await this.opts.vault.scan(this.opts.textExtensions, index);
    const currentByPath = new Map(current.map((file) => [file.path, file]));
    // Copy lazily: if the pull applies nothing, the original map is reused.
    let nextCurrentByPath: Map<string, LocalFileSnapshot> | null = null;
    const copyCurrentByPath = (): Map<string, LocalFileSnapshot> =>
      (nextCurrentByPath ??= new Map(currentByPath));
    const pathAccepted = this.currentPathMatcher();
    const pulledText = new Map<string, string>();
    const touched: LocalFileSnapshot[] = [];
    const deleted: string[] = [];

    try {
      for (const file of [...pull.added, ...pull.modified]) {
        this.throwIfAborted();
        if (!this.shouldApplyPulledPath(file.path, pathAccepted)) continue;
        const local = currentByPath.get(file.path);
        const indexed = index.files[file.path];
        if (isLocalDeleted(local, indexed?.lastSyncedHash)) {
          await this.writeRemoteConflict(file, pull.to);
          continue;
        }

        if (file.file_type === "text") {
          const content = await this.pulledTextContent(file, pull.to, pulledText);
          const { hash, byteLength } = await sha256TextWithLength(content);
          if (local?.kind === "text" && local.hash === hash) {
            touched.push(local);
            continue;
          }
          if (isLocalDirty(local, indexed?.lastSyncedHash)) {
            await this.writeConflict(file.path, local);
          }
          // The download/hash awaits above can straddle an abort (M-10
          // per-file granularity): re-check so a replaced generation
          // never writes into the vault it no longer owns.
          this.throwIfAborted();
          await this.opts.vault.writeText(file.path, content);
          touched.push({
            path: file.path,
            hash,
            size: byteLength,
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
          // Same as the text branch: the download/hash awaits can
          // straddle an abort, so re-check before the vault write.
          this.throwIfAborted();
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
        this.throwIfAborted();
        if (!shouldSyncPath(path) || !pathAccepted(path)) continue;
        const local = currentByPath.get(path);
        const indexed = index.files[path];
        if (isLocalDirty(local, indexed?.lastSyncedHash)) {
          await this.writeConflict(path, local);
        }
        // writeConflict above awaits; an abort landing in that window
        // must still stop the trash (M-10 per-file granularity).
        this.throwIfAborted();
        await this.opts.vault.trash(path);
        deleted.push(path);
      }
    } catch (error) {
      await this.savePartialPullProgress(touched, deleted);
      throw error;
    }

    let appliedIndex: LocalIndex | null = null;
    const touchedSynced = touched.filter((file) => shouldSyncPath(file.path));
    const deletedApplied = deleted.filter((path) => shouldSyncPath(path) && pathAccepted(path));
    // Final index advance: the per-file loop's last await may have
    // straddled an abort (M-10) — a replaced generation must not move the
    // head the replacement engine is about to own.
    this.throwIfAborted();
    await this.opts.index.updateIndex((current) => {
      let next = markSynced(current, pull.to, touchedSynced);
      next = markDeleted(next, pull.to, deletedApplied);
      appliedIndex = next;
      return next;
    });
    index = appliedIndex ?? index;
    for (const file of touched) {
      if (shouldSyncPath(file.path)) {
        copyCurrentByPath().set(file.path, file);
      }
    }
    for (const path of deleted) {
      copyCurrentByPath().delete(path);
    }
    return this.scanPendingFrom(index, [...(nextCurrentByPath ?? currentByPath).values()]);
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
    touched: LocalFileSnapshot[],
    deleted: string[]
  ): Promise<void> {
    if (touched.length === 0 && deleted.length === 0) return;
    const touchedSynced = touched.filter((file) => shouldSyncPath(file.path));
    const deletedSynced = deleted.filter((path) => shouldSyncPath(path));
    await this.opts.index.updateIndex((current) => {
      let partial = markFilesSynced(current, touchedSynced);
      partial = markFilesDeleted(partial, deletedSynced);
      return partial;
    });
  }

  private async writeConflict(
    path: string,
    local: LocalFileSnapshot | undefined
  ): Promise<void> {
    if (!local) return;
    this.throwIfAborted();
    const cpath = conflictPath(path, this.opts.deviceName);
    if (local.kind === "text") {
      await this.opts.vault.writeText(cpath, local.content ?? "");
    } else if (local.bytes) {
      await this.opts.vault.writeBinary(cpath, local.bytes);
    }
    new Notice(`PKV Sync conflict: ${cpath}`);
  }

  private async writeRemoteConflict(file: PullFile, atCommit: string): Promise<void> {
    this.throwIfAborted();
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
    this.throwIfAborted();
    const { hash, byteLength: size } = await sha256TextWithLength(content);
    const snapshot: LocalFileSnapshot = { path, hash, size, kind: "text", content };
    await this.opts.index.updateIndex(async (index) => {
      const indexed = index.files[path];
      const fileExists = this.opts.vault.exists(path);
      // Presence mismatch means unknown local state (audit H-4):
      // - local file exists but has no index entry: its content was never
      //   pushed, so it exists nowhere else — overwriting it with remote
      //   content would silently destroy it;
      // - index entry exists but the local file is gone: the user deleted
      //   it, and applying remote content here would silently resurrect it.
      // Both refuse and fall back to the full pull, which surfaces the
      // remote content as a .conflict-* copy instead of clobbering. The
      // both-absent case (a genuinely new remote file) has nothing local
      // to lose and still applies directly.
      if (fileExists !== (indexed !== undefined)) {
        throw new InlineApplyDirtyError(path);
      }
      if (indexed) {
        const localContent = await this.opts.vault.readText(path);
        const localHash = await sha256Text(localContent);
        if (localHash !== indexed.lastSyncedHash) {
          // Local has unsynced edits. Throwing inside updateIndex aborts
          // the data-store update without writing, so the index does not
          // advance and the file is not touched.
          throw new InlineApplyDirtyError(path);
        }
        // Close the TOCTOU window between the dirty check and writeText:
        // re-read and re-hash. If the file changed on disk between the two
        // reads (user edit landing in the gap), the index would be advanced
        // over a stale view — refuse instead of clobbering the edit.
        const recheckContent = await this.opts.vault.readText(path);
        const recheckHash = await sha256Text(recheckContent);
        if (recheckHash !== localHash) {
          throw new InlineApplyDirtyError(path);
        }
      }
      // Write file inside the atomic update so a concurrent applyInlineText
      // cannot interleave its writeText between our dirty check and our save.
      // The dirty-check reads above await; an abort landing in that window
      // must still stop the write (M-10 per-file granularity).
      this.throwIfAborted();
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
    this.throwIfAborted();
    await this.opts.index.updateIndex(async (index) => {
      const indexed = index.files[path];
      const fileExists = this.opts.vault.exists(path);
      // Same reasoning as applyInlineText (audit H-4): a path with no index
      // entry is a local file we know nothing about (never pushed), so its
      // content is not recorded anywhere else. Refuse the inline delete and
      // fall back to the full pull, which trashes only paths whose local
      // state it could verify (and writes a conflict copy otherwise). A
      // path that is neither indexed nor locally present has nothing to
      // lose and still deletes (a no-op trash).
      if (fileExists && indexed === undefined) {
        throw new InlineApplyDirtyError(path);
      }
      if (indexed && fileExists) {
        const local = await this.opts.vault.snapshot(path, this.opts.textExtensions);
        if (isLocalDirty(local, indexed.lastSyncedHash)) {
          await this.writeConflict(path, local);
          return index;
        }
      }
      // The snapshot await above can straddle an abort (M-10 per-file
      // granularity): re-check before the trash.
      this.throwIfAborted();
      await this.opts.vault.trash(path);
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
