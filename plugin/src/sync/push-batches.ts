import { ApiError } from "../api/client";
import { pushBodyBytes, pushChangeBytes } from "../api/push-request";
import type { PushChange, PushResponse } from "./types";
import type { PushFailure } from "./push-errors";
import {
  DEFAULT_PUSH_TARGET_BYTES,
  MAX_413_FAILURES,
  QUARANTINE_RETRY_MS,
  SINGLETON_RETRY_MS,
  normalizePushLimits,
  type PushLimits
} from "./push-limits";

export interface PushItem {
  change: PushChange;
  fingerprint: string;
}

export interface PushCallbacks<T extends PushItem> {
  deviceName: string;
  send(items: readonly T[]): Promise<PushResponse>;
  onAccepted(items: readonly T[], response: PushResponse): Promise<void>;
  /**
   * R15-2: a deterministic single-file rejection took this path out of the
   * batch. The file is still on disk and still pending locally — it will
   * never reach the server — so the user has to be told which one and why.
   */
  onQuarantined?(path: string, code: string): void;
}

export interface PushRun {
  send<T extends PushItem>(
    items: readonly T[],
    callbacks: PushCallbacks<T>
  ): Promise<void>;
}

interface RejectedItem {
  fingerprint: string;
  bytes: number;
  until: number;
}

/**
 * R15-2: server rejections that are a property of the PATH or its CONTENT,
 * not of the request. Retrying the same bytes can never succeed, so a single
 * such file must not be able to fail the run that carries it.
 *
 * R17-H2: the list is the server's per-file error contract, spelled out.
 * Four codes describe the path or its classification:
 * - `invalid_path`: path::normalize refused the name (over-deep percent
 *   chain, reserved name, escaped separator, over-long component…).
 * - `invalid_text`: the text body parses as a blob pointer in its
 *   entirety (R14-M4).
 * - `path_excluded`: the vault's exclude globs or allowlist no longer
 *   admit this path.
 * - `wrong_file_kind`: the admin's text/binary classifier disagrees with
 *   what we sent.
 * Five more are per-file verdicts produced while merging one change:
 * - `file_too_large`: this file exceeds max_file_size.
 * - `path_conflict`: the path collides with a case-folded sibling or an
 *   existing directory (S5 guard).
 * - `missing_blob`: the blob this change points at is not uploaded.
 * - `blob_size_mismatch`: the uploaded blob's size disagrees with the
 *   declared size.
 * - `bad_commit`: the base/head tree cannot be read at this path.
 * All nine are decided by the bytes we sent — no retry of the same
 * request can change the answer.
 *
 * Anything else — 401/403 (a revoked token), 429 (a retryable throttle),
 * 409 (head mismatch, which the engine retries at a higher level), 5xx,
 * an unknown `code` — still throws: those say something about the REQUEST
 * or the SESSION, and swallowing them would record a batch as applied
 * that the server never accepted.
 */
const DETERMINISTIC_4XX_CODES: ReadonlySet<string> = new Set([
  "invalid_path",
  "invalid_text",
  "path_excluded",
  "wrong_file_kind",
  // R17-H2: per-file merge verdicts. Isolating them is what keeps one
  // oversized file, one case-folded collision, or one un-downloadable
  // blob from wedging every other pending change in the vault behind a
  // permanently red status.
  "file_too_large",
  "path_conflict",
  "missing_blob",
  "blob_size_mismatch",
  "bad_commit"
]);

function deterministicCode(error: unknown): string | null {
  if (!(error instanceof ApiError)) return null;
  if (error.status !== 400) return null;
  return DETERMINISTIC_4XX_CODES.has(error.code) ? error.code : null;
}

/**
 * Pack one request's worth of items starting at `offset`.
 *
 * The rules are all here, once: the empty-body seed, the `+1` separator
 * between items, the target-byte stop, `limits.max_changes`, and a single
 * item that cannot fit at all (`hard_limit`). R15-2 inlined a copy of this
 * inside `sendPacked` while leaving the original as dead code, so the two
 * could drift; R16-10 asked for one or the other, and the skip-aware version
 * is the one that has to exist.
 *
 * `skip` withholds items (quarantined paths) without consuming them: the
 * caller advances to the returned `end`, which is the exclusive end of the
 * window this call examined — items before it were either packed or
 * deliberately skipped, so nothing is retried and nothing is jumped over.
 */
function packChunk<T extends PushItem>(options: {
  items: readonly T[];
  offset: number;
  deviceName: string;
  target: number;
  limits: PushLimits;
  weight: (item: T) => number;
  error: (failure: PushFailure) => Error;
  skip?: (item: T) => boolean;
}): { chunk: T[]; end: number; bytes: number } {
  const { items, offset, deviceName, target, limits, weight, error, skip } = options;
  const chunk: T[] = [];
  let bytes = pushBodyBytes([], deviceName);
  let next = offset;
  while (next < items.length && chunk.length < limits.max_changes) {
    const item = items[next];
    if (skip?.(item)) {
      next += 1;
      continue;
    }
    const candidate = bytes + weight(item) + (chunk.length ? 1 : 0);
    if (chunk.length && (candidate > target || candidate > limits.max_json_bytes)) {
      break;
    }
    if (candidate > limits.max_json_bytes) {
      throw error({
        kind: "hard_limit",
        path: item.change.path,
        bytes: candidate,
        limit: limits.max_json_bytes
      });
    }
    chunk.push(item);
    bytes = candidate;
    next += 1;
    if (bytes > target) break;
  }
  return { chunk, end: next, bytes };
}

function splitAtByteMiddle<T extends PushItem>(
  items: readonly T[],
  weight: (item: T) => number
): [T[], T[]] {
  const total = items.reduce((sum, item) => sum + weight(item) + 1, 0);
  let prefix = 0;
  let cut = 1;
  let best = Number.POSITIVE_INFINITY;
  for (let i = 0; i < items.length - 1; i++) {
    prefix += weight(items[i]) + 1;
    const distance = Math.abs(total - 2 * prefix);
    if (distance < best) {
      best = distance;
      cut = i + 1;
    }
  }
  return [items.slice(0, cut), items.slice(cut)];
}

export class PushBatcher {
  private target = DEFAULT_PUSH_TARGET_BYTES;
  private limitsKey = "";
  private deferredUntil = 0;
  private rejected = new Map<string, RejectedItem>();
  /**
   * R15-2: paths the server deterministically refuses. Kept separately from
   * `rejected` (which models "too big for the proxy" and therefore carries a
   * byte weight) because the reason and the retry horizon differ: a proxy
   * limit can lift, a rejected path cannot change without the file changing.
   */
  private quarantined = new Map<string, { fingerprint: string; code: string; until: number }>();
  // R8-5: a manual retry can arrive while a run is already mid-send (its
  // beginRun consumed the entry probe). Consuming this flag per send
  // clears the learned cooldowns from that point on, so the user's
  // explicit "try again" takes effect in the pass it coalesced onto —
  // not only in some later automatic pass.
  private midRunRetryRequested = false;

  constructor(
    private readonly makeError: (failure: PushFailure) => Error,
    private readonly now: () => number = () => Date.now()
  ) {}

  /** Arm a mid-run retry: the next send (even in the current run) retries cooled files. */
  requestMidRunRetry(): void {
    this.midRunRetryRequested = true;
  }

  beginRun(
    input: PushLimits,
    assertActive: () => void,
    retryOversized = false
  ): PushRun {
    const limits = normalizePushLimits(input);
    const key = JSON.stringify(limits);
    if (key !== this.limitsKey) {
      this.limitsKey = key;
      this.target = Math.min(DEFAULT_PUSH_TARGET_BYTES, limits.max_json_bytes);
      this.rejected.clear();
      this.deferredUntil = 0;
    }
    if (retryOversized) {
      this.rejected.clear();
      this.deferredUntil = 0;
      // A manual "try again" also re-tries quarantined files: the user may
      // have fixed the configuration (changed the allowlist, renamed the
      // file) since they were withheld.
      this.quarantined.clear();
    }
    for (const [path, entry] of this.rejected) {
      if (entry.until <= this.now()) this.rejected.delete(path);
    }
    // A quarantine expires with the fingerprint, not with the clock alone:
    // an unchanged file stays out (re-sending it is a guaranteed 400), an
    // edited one is retried.
    for (const [path, entry] of this.quarantined) {
      if (entry.until <= this.now()) this.quarantined.delete(path);
    }
    assertActive();
    if (this.deferredUntil > this.now()) {
      throw this.makeError({ kind: "retry_after", deadline: this.deferredUntil });
    }
    let failures = 0;
    return {
      send: async <T extends PushItem>(
        items: readonly T[],
        callbacks: PushCallbacks<T>
      ): Promise<void> => {
        const emptyBytes = pushBodyBytes([], callbacks.deviceName);
        const weights = new Map<T, number>();
        const weight = (item: T): number => {
          const cached = weights.get(item);
          if (cached !== undefined) return cached;
          const bytes = pushChangeBytes(item.change);
          weights.set(item, bytes);
          return bytes;
        };
        const sendPacked = async (remaining: readonly T[]): Promise<void> => {
          let offset = 0;
          while (offset < remaining.length) {
            assertActive();
            // R8-5: consume the mid-run retry here so a manual retry
            // arriving after this run's beginRun still takes effect on
            // the very next chunk this run sends.
            if (this.midRunRetryRequested) {
              this.midRunRetryRequested = false;
              this.rejected.clear();
              this.deferredUntil = 0;
              this.quarantined.clear();
            }
            // R15-2: skip every path the server has already refused for
            // these exact bytes. R16-10: the packing rules live in
            // `packChunk`, once — this loop used to carry a second copy of
            // them while the original sat unused.
            const withheld: Array<{ path: string; code: string }> = [];
            const packed = packChunk({
              items: remaining,
              offset,
              deviceName: callbacks.deviceName,
              target: this.target,
              limits,
              weight,
              error: this.makeError,
              skip: (item) => {
                const code = this.quarantineCode(item);
                if (code === null) return false;
                withheld.push({ path: item.change.path, code });
                return true;
              }
            });
            const { chunk, bytes } = packed;
            const next = packed.end;
            // R17-H3: a withheld path produces no send and no rejection in
            // THIS pass, so from the second pass on the caller's per-pass
            // refusal map came back empty and the vault painted itself
            // green while the file stayed missing from the server. The
            // refusal is a state, not an event: report every withheld path
            // on every pass that ends without syncing it. Duplicate
            // notices are the caller's concern (one per path per pass).
            for (const item of withheld) {
              callbacks.onQuarantined?.(item.path, item.code);
            }
            if (chunk.length === 0) {
              // Nothing sendable left in this window (everything from
              // `offset` on is quarantined).
              break;
            }
            for (const item of chunk) {
              const previous = this.rejected.get(item.change.path);
              const singletonBytes = emptyBytes + weight(item);
              if (
                previous &&
                previous.until > this.now() &&
                previous.bytes === singletonBytes &&
                previous.fingerprint === item.fingerprint
              ) {
                throw this.makeError({
                  kind: "remote_singleton",
                  path: item.change.path,
                  bytes: singletonBytes
                });
              }
            }
            let response: PushResponse;
            try {
              assertActive();
              response = await callbacks.send(chunk);
            } catch (error) {
              assertActive();
              const code = deterministicCode(error);
              if (code !== null) {
                if (chunk.length === 1) {
                  // A single file the server refuses by name. Quarantine it
                  // and carry on with the rest of `remaining` instead of
                  // failing the run that carries it (R15-2).
                  const item = chunk[0];
                  this.quarantined.set(item.change.path, {
                    fingerprint: item.fingerprint,
                    code,
                    until: this.now() + QUARANTINE_RETRY_MS
                  });
                  callbacks.onQuarantined?.(item.change.path, code);
                  offset = next;
                  continue;
                }
                // Bisect to find the offender; the healthy half is still
                // sent (the recursion quarantines on the way down).
                const [left, right] = splitAtByteMiddle(chunk, weight);
                assertActive();
                await sendPacked(left);
                assertActive();
                await sendPacked(right);
                offset = next;
                continue;
              }
              if (!(error instanceof ApiError) || error.status !== 413) throw error;
              if (error.retryAfterAt !== undefined && error.retryAfterAt > this.now()) {
                this.deferredUntil = error.retryAfterAt;
                throw this.makeError({
                  kind: "retry_after",
                  deadline: this.deferredUntil
                });
              }
              failures += 1;
              if (chunk.length === 1) {
                const item = chunk[0];
                this.rejected.set(item.change.path, {
                  fingerprint: item.fingerprint,
                  bytes,
                  until: this.now() + SINGLETON_RETRY_MS
                });
                throw this.makeError({
                  kind: "remote_singleton",
                  path: item.change.path,
                  bytes
                });
              }
              if (failures >= MAX_413_FAILURES) {
                throw this.makeError({ kind: "retry_budget", failures });
              }
              this.target = Math.min(
                this.target,
                Math.max(emptyBytes + 1, Math.floor(bytes / 2))
              );
              const [left, right] = splitAtByteMiddle(chunk, weight);
              if (
                pushBodyBytes(
                  left.map((item) => item.change),
                  callbacks.deviceName
                ) >= bytes ||
                pushBodyBytes(
                  right.map((item) => item.change),
                  callbacks.deviceName
                ) >= bytes
              ) {
                throw new Error("push_partition_did_not_shrink");
              }
              assertActive();
              await sendPacked(left);
              assertActive();
              await sendPacked(right);
              offset = next;
              continue;
            }
            assertActive();
            await callbacks.onAccepted(chunk, response);
            assertActive();
            offset = next;
          }
        };
        await sendPacked(items);
      }
    };
  }

  /**
   * The active quarantine code for this item's exact content, or null when
   * the path is not withheld. Split out of `isQuarantined` so the skip
   * path can report WHICH code withheld the file (R17-H3).
   */
  private quarantineCode(item: PushItem): string | null {
    const entry = this.quarantined.get(item.change.path);
    if (!entry) return null;
    if (entry.until <= this.now()) {
      this.quarantined.delete(item.change.path);
      return null;
    }
    // The cooldown only covers the CONTENT the server refused. An edited
    // file has a new fingerprint and is worth a retry immediately.
    if (entry.fingerprint !== item.fingerprint) return null;
    return entry.code;
  }

  private isQuarantined(item: PushItem): boolean {
    return this.quarantineCode(item) !== null;
  }
}
