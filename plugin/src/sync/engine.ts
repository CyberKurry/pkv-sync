import { Notice } from "obsidian";
import { PromiseChain } from "../util/promise-chain";
import { ApiError } from "../api/client";
import { SseAuthError, subscribeVaultEvents } from "../api/events-client";
import type { SyncApi } from "../api/sync-client";
import type { VaultEvent } from "../api/types";
import type { VaultSettings } from "../api/types";
import { format, strings, type Strings } from "../i18n";
import { createPathMatcher } from "./exclude";
import { conflictPath } from "./conflict";
import { sha256Bytes, sha256Text, sha256TextWithLength } from "./hash";

import { pushItemFingerprint, snapshotToPushChange } from "./push-prepare";
import {
  markBatch,
  markDeleted,
  markFilesDeleted,
  markFilesSynced,
  markSynced
} from "./index-store";
import type { LocalFileSnapshot, LocalIndex, PullFile, PullResponse, PushResponse } from "./types";
import {
  PushBatcher,
  type PushItem,
  type PushRun
} from "./push-batches";
import { PushBatchError, type PushLabels } from "./push-errors";
import { LEGACY_PUSH_LIMITS, type PushLimits } from "./push-limits";
import { debugLog, errorToMessage } from "../util";
import {
  shouldAcceptRemoteConflictPath,
  shouldSyncPath,
  type VaultAdapter
} from "./vault-adapter";

const BLOB_UPLOAD_CONCURRENCY = 4;
// R9-Dup-6: derive from LEGACY_PUSH_LIMITS — the server's own 1000
// (service/sync/limits.rs) is the wire contract and stays independent.
const MAX_PUSH_CHANGES = LEGACY_PUSH_LIMITS.max_changes;

interface PendingScan {
  pending: LocalFileSnapshot[];
  deleted: string[];
  index: LocalIndex;
  /** D3: the breaker held every local deletion this pass (spec §3.4). */
  emptyScanHeld?: boolean;
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
  labels?: Pick<
    Strings,
    | "inlineApplyFailed"
    | "conflictNotice"
    | "conflictCopyFailed"
    | "sseAuthFailed"
    | "pushPathRejected"
    | "pushPathsRefusedSummary"
    | "emptyScanHeld"
  >;
  onSyncSuccess?(): void | Promise<void>;
  vaultSettingsReader?: (vaultId: string) => Promise<VaultSettings>;
  getPushLimits?(): Readonly<PushLimits>;
  pushLabels?: PushLabels;
}

type EnginePushItem = PushItem &
  (
    | { source: "file"; file: LocalFileSnapshot }
    | { source: "delete"; deletedPath: string }
  );

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
  private readonly eventChain = new PromiseChain();
  /**
   * The in-flight (or queued) syncNow pass. Present while a pass is on the
   * chain so additional syncNow calls coalesce onto it instead of queueing
   * another pass behind it.
   */
  private activeSync: Promise<void> | null = null;

  /**
   * Learned push batching state (target size, rejected single files,
   * Retry-After deadline) shared by every pass of this engine generation.
   */
  private pushBatcherInstance: PushBatcher | null = null;
  private oversizedProbeRequested = false;
  /**
   * D3 breaker escape hatch (spec §3.4): set by the explicit "confirm
   * empty-vault deletions" command. Consumed by the NEXT sync pass only —
   * a held pass must never stay overrideable forever by accident.
   *
   * R15-7: "the next pass" has to mean a pass that starts AFTER the
   * confirmation, not the pass that happens to run next. A confirmation
   * clicked while a pass is in flight coalesced onto that pass, which was
   * already past the consumption point, so the armed flag outlived the pass
   * it was meant for and was lifted by some later, unrelated one — a
   * whole-vault deletion the user never confirmed for that moment. What
   * makes the two distinguishable is `emptyScanConfirmCoalesced` right
   * below: a click that lands on an in-flight pass records itself there,
   * and `syncNow`'s completion hook runs the pass it actually asked for.
   */
  private emptyScanOverride = false;
  /**
   * R15-7: a confirmation clicked WHILE a pass is in flight cannot be
   * honoured by that pass — it is long past the consumption point — so
   * something has to run a pass that starts after it. `emptyScanConfirmCoalesced`
   * records that the click landed on an in-flight pass; `syncNow`'s
   * completion hook sees it and runs one more pass, which is the pass the
   * user's click actually asked for.
   */
  private emptyScanConfirmCoalesced = false;
  /**
   * The override value lifted for the pass currently in flight: every
   * scanPendingFrom classification inside one pass — including the 409
   * head-mismatch retry's trailing reclassification — shares it, so a
   * confirmed pass cannot be split into an override half and a held half.
   */
  private passOverride = false;
  /** Deletions held by the breaker on the most recent completed classification. */
  private lastHeldDeletionCount = 0;

  constructor(private opts: SyncEngineOptions) {
    if (!opts.vaultId.trim()) throw new Error("SyncEngine requires a non-empty vaultId");
    if (!opts.deviceName.trim()) {
      throw new Error("SyncEngine requires a non-empty deviceName");
    }
  }

  private get pushBatcher(): PushBatcher {
    this.pushBatcherInstance ??= new PushBatcher(
      (failure) => new PushBatchError(failure, this.pushLabels())
    );
    return this.pushBatcherInstance;
  }

  private pushLabels(): PushLabels {
    return this.opts.pushLabels ?? strings();
  }

  private consumeOversizedProbe(): boolean {
    const requested = this.oversizedProbeRequested;
    this.oversizedProbeRequested = false;
    return requested;
  }

  /** Cancel any in-flight sync pass and future event handling (M-10). */
  abort(): void {
    this.abortController.abort();
  }

  /** Arm the D3 escape hatch (spec §3.4) for the next sync pass. */
  allowEmptyScanDeletesOnce(): void {
    this.emptyScanOverride = true;
    // R15-7: if a pass is already in flight, it has passed the consumption
    // point and cannot honour this click. Note it so the completion hook
    // schedules the pass the click actually asked for; otherwise the flag
    // would survive into whatever unrelated pass ran next.
    if (this.activeSync) this.emptyScanConfirmCoalesced = true;
  }

  /**
   * Deletions held by the breaker on the most recent completed
   * classification (0 when none).
   */
  heldDeletionCount(): number {
    return this.lastHeldDeletionCount;
  }

  /**
   * R15-7: whether a "confirm empty-vault deletions" click is still waiting
   * for a pass to actually start. Observable so the completion hook's
   * follow-up pass can be asserted instead of inferred from timing.
   */
  emptyScanConfirmPending(): boolean {
    return this.emptyScanOverride || this.emptyScanConfirmCoalesced;
  }

  private get aborted(): boolean {
    return this.abortController.signal.aborted;
  }

  /** Throws the typed cancellation when this generation was aborted. */
  private throwIfAborted(): void {
    if (this.aborted) throw new SyncAbortedError();
  }

  async syncNow(options: { retryOversized?: boolean } = {}): Promise<void> {
    if (options.retryOversized) this.oversizedProbeRequested = true;
    if (this.activeSync) {
      // R8-5: the user's explicit "try again" must not be deferred behind
      // a coalesced pass that already consumed the entry probe (it is
      // past beginRun). Poke the batcher so the in-flight run retries the
      // cooled files from its next chunk on; the deferredUntil gate is
      // cleared too, matching what a fresh retry pass would do.
      if (options.retryOversized) this.pushBatcher.requestMidRunRetry();
      return this.activeSync;
    }
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
        this.runDeferredEmptyScanConfirm();
      },
      (error) => {
        if (this.activeSync === completion) this.activeSync = null;
        this.runDeferredEmptyScanConfirm();
        throw error;
      }
    );
    this.activeSync = completion;
    return completion;
  }

  /**
   * R15-7: honour a "confirm empty-vault deletions" click that arrived while
   * a pass was in flight.
   *
   * The click could not be consumed by the running pass (it had already
   * lifted the override), so it is still armed. The pass the user asked for
   * has to start now — otherwise the flag survives into whatever unrelated
   * pass happens to run next, which is a whole-vault deletion nobody
   * confirmed for that moment. Deliberately fire-and-forget: the click's own
   * caller (`void plugin.syncNowManual()`) has already returned, and the
   * completion of this pass must not be delayed by the next one.
   */
  private runDeferredEmptyScanConfirm(): void {
    if (!this.emptyScanConfirmCoalesced) return;
    this.emptyScanConfirmCoalesced = false;
    // An aborted/replaced generation owns neither the vault nor the status
    // element any more; the replacement engine's own sync covers it.
    if (this.aborted) return;
    if (!this.emptyScanOverride) return;
    void this.syncNow().catch(() => undefined);
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
    // The chain absorbs rejections internally; the caller sees `run`.
    return this.eventChain.enqueue(task);
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
          if (!(await this.reconcileEventPosition(ev))) return;
          let needFallbackPull = false;
          // R15-6: the local exclusion rules are a promise about THIS
          // device's disk, so every surface that writes the disk must
          // honour them — not only the pull. The SSE loop called the apply
          // helpers directly, so a vault whose local excludes are narrower
          // than the server's (`.obsidian/plugins/**` kept out of sync on
          // purpose) had those files trashed by a remote delete: a local
          // exclusion silently overridden by the realtime stream. One rule,
          // one predicate: the pull's shouldApplyPulledPath.
          const pathAccepted = this.currentPathMatcher();
          for (const change of ev.changes) {
            try {
              switch (change.kind) {
                case "text_inline":
                  if (!this.shouldApplyPulledPath(change.path, pathAccepted)) break;
                  await this.applyInlineText(change.path, change.content);
                  break;
                case "delete":
                  if (!this.shouldApplyPulledPath(change.path, pathAccepted)) break;
                  await this.applyDelete(change.path);
                  break;
                case "text_ref":
                case "blob":
                  needFallbackPull = true;
                  break;
              }
            } catch (err) {
              // An abort mid-apply is not a failure (R2-7): the engine
              // generation was replaced and the replacement engine owns
              // the vault, the index AND the status element from here on.
              // Swallow silently — no Notice, no error status, no fallback
              // pull (the replacement engine's own sync covers the missed
              // commits) — the same contract syncInner already honors.
              if (err instanceof SyncAbortedError) {
                return;
              }
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
        if (err instanceof SseAuthError) {
          // W5: the stream is stopped for good (revoked/expired token).
          // The poll timer keeps the vault syncing, but realtime stays
          // off until the credentials are fixed — say so instead of
          // pretending reconnects will continue.
          debugLog("[pkv-sync] SSE stream stopped (authentication):", err);
          this.opts.setStatus(
            "error",
            (this.opts.labels ?? strings()).sseAuthFailed
          );
          return;
        }
        debugLog("[pkv-sync] SSE event stream error; automatic reconnect will continue:", err);
      },
    });
  }

  stopEventSubscription(): void {
    this.unsubscribeEvents?.();
    this.unsubscribeEvents = null;
  }

  async scanPending(): Promise<PendingScan> {
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
    // R11-2: the deletion filter must consult the SAME rule the scan
    // listing applies (shouldSyncPath). The scan already dropped
    // escaped-separator paths (the server's R10-1 twin), so an index
    // entry for one reads as "missing from the scan" here — classifying
    // it as a local delete would emit a delete for a file the user
    // still has (the data-loss direction R9-7's ruling forbids). A path
    // the sync plan refuses on both sides is neither pending nor
    // deleted; it simply stays out of the conversation.
    const deleted = Object.keys(index.files).filter(
      (p) => !currentPaths.has(p) && pathAccepted(p) && shouldSyncPath(p)
    );
    // D3 (spec §3.4): a scan that lists ZERO files against a populated
    // index means "the vault looks empty", not "the user deleted
    // everything" — cold-start cache, revoked storage permission, an
    // unmounted drive. Pushing would broadcast a whole-vault deletion
    // (verified: this exact shape pushes N delete changes today). Hold
    // every deletion; the explicit confirm command is the only way through.
    const vaultScannedEmpty = current.length === 0;
    const indexHasEntries = Object.keys(index.files).length > 0;
    if (
      vaultScannedEmpty &&
      indexHasEntries &&
      // nothing classifiable as a deletion: holding would only pin an error status
      deleted.length > 0 &&
      !this.passOverride
    ) {
      this.lastHeldDeletionCount = deleted.length;
      return { pending, deleted: [], index, emptyScanHeld: true };
    }
    this.lastHeldDeletionCount = 0;
    return { pending, deleted, index };
  }

  private matcherCache = new Map<string, (path: string) => boolean>();

  private currentPathMatcher(): (path: string) => boolean {
    const userExcludes = this.opts.extraExcludeGlobs ?? [];
    const userAllowlist =
      this.vaultSettingsCache.get(this.opts.vaultId)?.extra_sync_globs ?? [];
    const configDir = this.opts.vault.configDir;
    // Compile the glob matcher once per settings value (exact key, no TTL) so
    // the same exclude/allowlist sets are not recompiled on every sync pass.
    const key = JSON.stringify([userExcludes, userAllowlist, configDir]);
    let matcher = this.matcherCache.get(key);
    if (!matcher) {
      matcher = createPathMatcher({ userExcludes, userAllowlist, configDir });
      this.matcherCache.set(key, matcher);
    }
    return matcher;
  }

  private async syncInner(): Promise<void> {
    try {
      this.throwIfAborted();
      this.opts.setStatus("syncing");
      // D3: lift the armed confirm into a pass-scoped value BEFORE the
      // first classification runs, so every scanPendingFrom call in this
      // pass (the 409 head-mismatch retry's reclassification included)
      // honors the same decision instead of re-holding halfway through.
      //
      this.passOverride = this.emptyScanOverride;
      this.emptyScanOverride = false;
      await this.refreshVaultSettings();

      // ① scan BEFORE any pull
      const scan = await this.scanPending();
      const emptyScanHeld = scan.emptyScanHeld === true;
      this.throwIfAborted();

      // ② push-first: send pending changes before pulling
      const run = this.pushBatcher.beginRun(
        this.opts.getPushLimits?.() ?? LEGACY_PUSH_LIMITS,
        () => this.throwIfAborted(),
        this.consumeOversizedProbe()
      );
      const refused = await this.pushPendingWithHeadMismatchRetry(scan, run);
      this.throwIfAborted();

      // ③ pull backflow: merged content arrives as a normal pull
      await this.pullIfChanged();

      this.throwIfAborted();
      if (emptyScanHeld) {
        // A held pass is not a successful sync: keep the warning visible
        // and do not refresh "last synced" (spec §3.4).
        this.opts.setStatus("error", (this.opts.labels ?? strings()).emptyScanHeld);
      } else if (refused.size > 0) {
        // R16-H1: the pass moved everything it could, but some files were
        // refused and remain on this device only. Claiming a clean sync
        // here would advance lastSyncSuccessAt and paint the status bar
        // green over a vault that is quietly incomplete; the per-path
        // Notice already named them, so this keeps the summary honest
        // without repeating the list.
        this.opts.setStatus(
          "error",
          format((this.opts.labels ?? strings()).pushPathsRefusedSummary, {
            count: refused.size
          })
        );
      } else {
        this.opts.setStatus("connected");
        await this.opts.onSyncSuccess?.();
      }
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
    } finally {
      // An aborted/replaced pass must not leak its pass-scoped override
      // into a later pass of this engine.
      this.passOverride = false;
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

  private async pushPending(
    scan: PendingScan | null | undefined,
    run: PushRun
  ): Promise<Map<string, string>> {
    // R16-H1: paths the server refused this pass. They stay on this device
    // and are absent from the index, so the pass cannot claim a clean sync.
    const refused = new Map<string, string>();
    // R15-2: one Notice per quarantined path per pass — a path that stays
    // pending is re-scanned every pass, and re-quarantining it is expected.
    const reported = new Set<string>();
    const { pending, deleted, index } = scan ?? await this.scanPending();
    if (pending.length === 0 && deleted.length === 0) return refused;

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

      const prepared: EnginePushItem[] = [
        ...hydrated.map((file) => ({
          source: "file" as const,
          file,
          change: snapshotToPushChange(file),
          fingerprint: pushItemFingerprint(file)
        })),
        ...batchDeleted.map((path) => ({
          source: "delete" as const,
          deletedPath: path,
          change: { kind: "delete" as const, path },
          fingerprint: JSON.stringify(["delete", path])
        }))
      ];

      await run.send(prepared, {
        deviceName: this.opts.deviceName,
        send: (accepted) =>
          this.opts.api.push(
            this.opts.vaultId,
            ifMatch,
            accepted.map((item) => item.change),
            this.opts.deviceName
          ),
        onAccepted: async (accepted, response) => {
          const files = accepted.flatMap((item) =>
            item.source === "file" ? [item.file] : []
          );
          const paths = accepted.flatMap((item) =>
            item.source === "delete" ? [item.deletedPath] : []
          );
          ifMatch = await this.applyPushedBatch(response, files, paths, ifMatch);
        },
        // R16-H1: the withholding mechanism shipped without its reporting
        // half, so a refused file vanished from the vault under a GREEN
        // status, with lastSyncSuccessAt advanced and no notice anywhere —
        // worse than the wedged state it replaced, which at least showed
        // red. The callback is optional, which is why neither the type
        // checker nor the dead-export guard could see that this call site
        // never passed it.
        onQuarantined: (path, code) => {
          refused.set(path, code);
          if (reported.has(path)) return;
          reported.add(path);
          this.reportPathRefused(path, code);
        }
      });
    }
    return refused;
  }

  private async applyPushedBatch(
    response: PushResponse,
    files: LocalFileSnapshot[],
    deleted: string[],
    ifMatch: string | null
  ): Promise<string | null> {
    // `merge_outcomes` is present ONLY when the server took the auto-merge
    // path, which by definition means the server head had already moved past
    // our base — commits we never pulled live in (base, new_commit]. The
    // per-path outcomes only describe the paths WE pushed; "clean" says
    // nothing about those missed commits. So the mere presence of the field
    // means the head must stay at the old base and the pull in ③ must cover
    // the merge range (audit H-1: advancing on all-clean left remote changes
    // permanently missed, since pull(since=new_commit) returns empty).
    // Array.isArray rather than a !== undefined check (R2-9): this
    // repository's server guarantees "absent or array" via
    // skip_serializing_if, but a third-party implementation sending
    // `"merge_outcomes": null` would otherwise be treated as auto-merged —
    // holding the head back and forcing an extra pull round for nothing.
    if (!Array.isArray(response.merge_outcomes)) {
      // Fast path (no auto-merge) — advance head immediately
      await this.opts.index.updateIndex((current) =>
        markBatch(current, response.new_commit, files, deleted, true)
      );
      return response.new_commit;
    }
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
        .filter((outcome) => outcome.outcome === "clean")
        .map((outcome) => outcome.path)
    );
    const cleanDeleted = deleted.filter((path) => cleanPaths.has(path));
    await this.opts.index.updateIndex((current) =>
      markBatch(current, current.lastSyncedCommit, files, cleanDeleted, false)
    );
    return ifMatch;
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
    scan: PendingScan | null,
    run: PushRun
  ): Promise<Map<string, string>> {
    // R16-H1: a 409 retry reclassifies and re-pushes, so the refusals from
    // the first attempt must survive into the answer the pass reports on.
    const refused = new Map<string, string>();
    const merge = (next: Map<string, string>) => {
      for (const [path, code] of next) refused.set(path, code);
    };
    try {
      merge(await this.pushPending(scan, run));
    } catch (error) {
      if (
        error instanceof ApiError &&
        error.status === 409 &&
        error.code === "head_mismatch"
      ) {
        const retryScan = await this.pullIfChanged();
        merge(await this.pushPending(retryScan, run));
        return refused;
      }
      throw error;
    }
    return refused;
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
    // Paths whose local deletion the server already rejected: the pull
    // reports them as modified while no local file exists, meaning a
    // delete-vs-modify verdict landed against us. They keep a .conflict-*
    // sidecar of the remote content and MUST leave the index with the
    // head advance — otherwise the next scanPendingFrom reclassifies the
    // missing file as a fresh local delete and re-sends it, which the
    // server's fast path (if_match == head) would then execute
    // unconditionally, destroying the other device's committed edit
    // vault-wide (S2).
    const deleteRejected: string[] = [];

    try {
      for (const file of [...pull.added, ...pull.modified]) {
        this.throwIfAborted();
        if (!this.shouldApplyPulledPath(file.path, pathAccepted)) continue;
        const local = currentByPath.get(file.path);
        const indexed = index.files[file.path];
        if (isLocalDeleted(local, indexed?.lastSyncedHash)) {
          try {
            await this.writeRemoteConflict(file, pull.to);
          } catch (error) {
            // W3: a sidecar the filesystem refuses (sidecar name longer
            // than the path limit, read-only directory) must not wedge the
            // vault at this commit forever. Nothing local is at stake here
            // — the local file is already gone — and the remote content
            // remains in the server history, so accept the verdict after
            // telling the user. An abort is not a failure and must
            // propagate.
            if (error instanceof SyncAbortedError) throw error;
            this.reportSidecarFailure(file.path, error);
          }
          deleteRejected.push(file.path);
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
          // R15-5: the dirty verdict above came from the scan snapshot,
          // taken before this file's download/hash. Re-read it: an edit
          // landing in that gap must be captured in a sidecar, not lost
          // under the remote write.
          if (!(await this.mayStillChangeDisk(file.path, local?.hash))) continue;
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
          // R15-5: same re-read as the text branch — the blob download is
          // the longest window of all.
          if (!(await this.mayStillChangeDisk(file.path, local?.hash))) continue;
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
        // R15-5: the delete decision above is the most destructive one in
        // the engine — trash destroys the bytes. Re-read the file: an edit
        // landing after the snapshot is captured in a sidecar instead.
        if (!(await this.mayStillChangeDisk(path, local?.hash))) continue;
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
    const rejectedApplied = deleteRejected.filter(
      (path) => shouldSyncPath(path) || shouldAcceptRemoteConflictPath(path)
    );
    // Final index advance: the per-file loop's last await may have
    // straddled an abort (M-10) — a replaced generation must not move the
    // head the replacement engine is about to own.
    this.throwIfAborted();
    await this.opts.index.updateIndex((current) => {
      let next = markSynced(current, pull.to, touchedSynced);
      next = markDeleted(next, pull.to, deletedApplied);
      // Server-rejected deletes drop their index entry under the same
      // head advance: the verdict is accepted, not re-litigated.
      next = markDeleted(next, next.lastSyncedCommit, rejectedApplied);
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

  /**
   * Conflict sidecars already written in this engine generation, keyed by
   * the conflict's content identity. A pull that fails on some other file
   * is retried from the same base, so the conflict branch runs again; the
   * timestamped sidecar name changes on every attempt, which used to stack
   * an unbounded number of copies of the same content (W2). Reusing the
   * copy while it still exists makes the retry idempotent; if the user
   * resolved it (the file is gone) the next attempt writes a fresh one.
   */
  /**
   * The index position a convergence sync reached in this generation, used
   * to coalesce a burst of stale replayed events into one sync (R7R2-1).
   * `null` = nothing converged yet; it is only compared against the current
   * head, so any head change makes it stale and re-arms the sync.
   */
  private convergedHead: string | null = null;

  private readonly writtenConflictSidecars = new Map<string, string>();

  private sidecarAlreadyWritten(key: string): boolean {
    const existing = this.writtenConflictSidecars.get(key);
    return existing !== undefined && this.opts.vault.exists(existing);
  }

  private async writeConflict(
    path: string,
    local: LocalFileSnapshot | undefined
  ): Promise<void> {
    if (!local) return;
    this.throwIfAborted();
    const key = `${path}\u0000local\u0000${local.hash}`;
    if (this.sidecarAlreadyWritten(key)) return;
    const cpath = conflictPath(path, this.opts.deviceName);
    if (local.kind === "text") {
      await this.opts.vault.writeText(cpath, local.content ?? "");
    } else if (local.bytes) {
      await this.opts.vault.writeBinary(cpath, local.bytes);
    }
    this.writtenConflictSidecars.set(key, cpath);
    new Notice(
      format((this.opts.labels ?? strings()).conflictNotice, { path: cpath })
    );
  }

  private async writeRemoteConflict(file: PullFile, atCommit: string): Promise<void> {
    this.throwIfAborted();
    const key = `${file.path}\u0000remote\u0000${atCommit}`;
    if (this.sidecarAlreadyWritten(key)) return;
    // R13-2: conflictPath now carries an always-on 8-char nonce (matching
    // the server's sidecar grammar), so two same-path conflicts from
    // different commits inside one second are unique by construction —
    // including across plugin reloads, which the previous in-memory
    // collision map could not cover.
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
    this.writtenConflictSidecars.set(key, cpath);
    new Notice(
      format((this.opts.labels ?? strings()).conflictNotice, { path: cpath })
    );
  }

  /**
   * R16-H1: tell the user, once per path per pass, that the server refused
   * this file and it is therefore staying on this device only.
   *
   * A refused path is a property of the path or its content, so no amount
   * of retrying changes it: the file is invisible to every other device
   * until the user renames or edits it. Withholding it silently was worse
   * than the wedged state R15-2 replaced, because the wedged state showed
   * red while this one showed green.
   */
  private reportPathRefused(path: string, code: string): void {
    const message = format((this.opts.labels ?? strings()).pushPathRejected, {
      path,
      reason: code
    });
    debugLog("[pkv-sync] the server refused this path; it stays local:", path, code);
    new Notice(message);
    this.opts.setStatus("error", message);
  }

  private reportSidecarFailure(path: string, error: unknown): void {
    const message = format(
      (this.opts.labels ?? strings()).conflictCopyFailed,
      { path, reason: errorToMessage(error) }
    );
    debugLog("[pkv-sync] conflict sidecar could not be written:", error);
    new Notice(message);
    this.opts.setStatus("error", message);
  }

  /**
   * Re-read a path and decide whether this engine may still change it.
   *
   * R15-5: four places decide "the local copy is clean, so the remote verdict
   * may overwrite it", and every one of them then awaits — a blob download, a
   * text fetch, a hash, or a whole batch of those. The file can change under
   * a verdict that was true when it was formed. `applyInlineText` and
   * `applyDelete` each grew their own re-read for this (R12-3, audit H-4);
   * the batch pull loop never did, and it has the widest window in the
   * codebase because one pull can span dozens of round trips.
   *
   * Returns true when the caller may proceed with the write or trash. Returns
   * false when the disk moved: the NEW content is captured in a conflict
   * sidecar, so the caller must leave the file alone and let the next pass
   * reconcile. `expectedHash` is the hash the caller's snapshot saw;
   * `undefined` means nothing was recorded about this path, so nothing can be
   * proven stale and the caller proceeds on its own rules.
   */
  private async mayStillChangeDisk(
    path: string,
    expectedHash: string | undefined
  ): Promise<boolean> {
    if (expectedHash === undefined) {
      // R16-H3: "the scan saw no local file" is NOT "there is no local
      // file". The scan happens once, at the top of applyPull, and this
      // call can be dozens of network round trips later — long enough for
      // the user to create the very path the pull is about to add. The
      // inline path has always guarded this shape ("presence mismatch
      // means unknown local state"); returning true here let the remote
      // body overwrite a file that had never been pushed anywhere.
      //
      // R16-7: the mirror case — the file existed at scan time and is gone
      // now — reaches the same place through a throw from `snapshot`. Both
      // are "the disk is not what the snapshot said", so both are answered
      // here, once.
      if (!this.opts.vault.exists(path)) return true;
      const appeared = await this.opts.vault.snapshot(path, this.opts.textExtensions);
      await this.writeConflict(path, appeared);
      return false;
    }
    // R16-7: a path deleted after the scan throws out of `snapshot`, which
    // failed the whole pass and pinned the head until the next one. A
    // vanished file is simply a disk that moved; treat it as such.
    if (!this.opts.vault.exists(path)) return false;
    const recheck = await this.opts.vault.snapshot(path, this.opts.textExtensions);
    if (recheck.hash === expectedHash) return true;
    await this.writeConflict(path, recheck);
    return false;
  }

  /**
   * The throwing half of [`mayStillChangeDisk`]. A caller that already has
   * the file's current hash (the inline paths read it as part of their own
   * dirty check) answers a mismatch by refusing, not by writing a sidecar:
   * its caller's recovery path is a full pull, which applies the sidecar
   * rule itself. Keeping both halves in one file — and both reading the disk
   * the same way — is what stops a third dialect of "is it still clean?"
   * from appearing.
   */
  private async stillMatchesOnDisk(path: string, expectedHash: string): Promise<boolean> {
    // R16-7: a file that vanished under us did not "still match".
    if (!this.opts.vault.exists(path)) return false;
    const recheck = await this.opts.vault.snapshot(path, this.opts.textExtensions);
    return recheck.hash === expectedHash;
  }

  private async applyInlineText(path: string, content: string): Promise<void> {
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
        // Here a refusal must THROW (not sidecar-and-skip): the caller is
        // the event loop, which answers an InlineApplyDirtyError with a
        // full pull that applies the same rule.
        if (!(await this.stillMatchesOnDisk(path, localHash))) {
          throw new InlineApplyDirtyError(path);
        }
      }
      // Write file inside the atomic update so a concurrent applyInlineText
      // cannot interleave its writeText between our dirty check and our save.
      // The dirty-check reads above await; an abort landing in that window
      // must still stop the write (M-10 per-file granularity).
      this.throwIfAborted();
      await this.opts.vault.writeText(path, content);
      // R14-H1: record the file WITHOUT advancing the head. The server's
      // pull is a tree-state diff — since === head returns empty — so an
      // inline apply that advanced lastSyncedCommit past its still-unapplied
      // siblings (text_ref/blob in the SAME event) stranded them forever.
      // The SSE loop advances the head once, at event level, only when the
      // whole event applied; a fallback pull then runs from ev.parent and
      // the tree diff re-delivers everything in range (already-applied
      // inline files re-apply as clean no-ops). Same discipline the push
      // path adopted for partial batches (markFilesSynced there).
      return markFilesSynced(index, [snapshot]);
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

  private async applyDelete(path: string): Promise<void> {
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
        // Captured before the dirty verdict narrows `local` (the predicate
        // is a type guard, so the false branch reads as "not a snapshot").
        const hashAtCheck: string | undefined = local?.hash;
        if (isLocalDirty(local, indexed.lastSyncedHash)) {
          await this.writeConflict(path, local);
          return index;
        }
        // R12-3 / R15-5: close the snapshot→trash TOCTOU window. An edit
        // landing between the snapshot (which showed the file clean, or
        // whose conflict copy was just written) and the trash would be
        // DESTROYED by the trash while the sidecar holds only the pre-edit
        // bytes. A mismatch writes a fresh sidecar capturing the new
        // content and skips the trash — the next pass reconciles.
        if (!(await this.mayStillChangeDisk(path, hashAtCheck))) {
          return index;
        }
      }
      // The snapshot await above can straddle an abort (M-10 per-file
      // granularity): re-check before the trash.
      this.throwIfAborted();
      await this.opts.vault.trash(path);
      // R14-H1: same event-level discipline as applyInlineText — drop the
      // index entry WITHOUT advancing the head; the SSE loop advances once
      // the whole event applied, and a fallback pull from ev.parent covers
      // the siblings this delete did not touch.
      return markFilesDeleted(index, [path]);
    });
  }

  /**
   * W1 + R7R2-1: decide whether an SSE event may be applied inline.
   *
   * `markSynced` records whatever commit it is handed — there is no
   * monotonicity check — so an event older than the current index position
   * (an SSE reconnect whose Last-Event-ID predates a poll that already
   * synced past it) must not be applied: it would overwrite newer content
   * with stale content and roll the head BACK.
   *
   * An event that does not extend the position exactly means the index is
   * out of step with the stream (stale replay, or a missed event); the
   * only safe move is a full sync, which converges either way. Because the
   * server replays up to MAX_SSE_REPLAY_COMMITS events, that decision can
   * fire once per replayed event; `convergedHead` remembers the position a
   * convergence sync reached, so a burst of stale events costs ONE sync
   * instead of one per event (each of which is a vault scan plus an HTTP
   * pull).
   *
   * Returns true when the event extends the position and may be applied.
   */
  private async reconcileEventPosition(ev: VaultEvent): Promise<boolean> {
    const index = await this.opts.index.loadIndex();
    if (ev.commit === index.lastSyncedCommit) return false;
    if (ev.parent !== index.lastSyncedCommit) {
      if (
        this.convergedHead !== null &&
        this.convergedHead === index.lastSyncedCommit
      ) {
        return false;
      }
      await this.syncNowInsideChainTask();
      // Read the position the pass reached; a failed pass throws before
      // this line, so `convergedHead` only ever records a real convergence.
      this.convergedHead = (await this.opts.index.loadIndex()).lastSyncedCommit;
      return false;
    }
    return true;
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
