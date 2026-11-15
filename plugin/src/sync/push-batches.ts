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
 * The list is deliberately closed and narrow. Anything else — 401/403 (a
 * revoked token), 429 (a retryable throttle), 409 (head mismatch, which the
 * engine retries at a higher level), 5xx, an unknown `code` — still throws:
 * those say something about the REQUEST or the SESSION, and swallowing them
 * would record a batch as applied that the server never accepted.
 */
const DETERMINISTIC_4XX_CODES: ReadonlySet<string> = new Set([
  // path::normalize refused the name (over-deep percent chain, reserved
  // name, escaped separator, over-long component…).
  "invalid_path",
  // The text body parses as a blob pointer in its entirety (R14-M4).
  "invalid_text",
  // The vault's exclude globs or allowlist no longer admit this path.
  "path_excluded",
  // The admin's text/binary classifier disagrees with what we sent.
  "wrong_file_kind"
]);

function deterministicCode(error: unknown): string | null {
  if (!(error instanceof ApiError)) return null;
  if (error.status !== 400) return null;
  return DETERMINISTIC_4XX_CODES.has(error.code) ? error.code : null;
}

function takePrefix<T extends PushItem>(
  items: readonly T[],
  offset: number,
  deviceName: string,
  target: number,
  limits: PushLimits,
  weight: (item: T) => number,
  error: (failure: PushFailure) => Error
): T[] {
  const chunk: T[] = [];
  let bytes = pushBodyBytes([], deviceName);
  for (let i = offset; i < items.length && chunk.length < limits.max_changes; i++) {
    const next = bytes + weight(items[i]) + (chunk.length ? 1 : 0);
    if (chunk.length && (next > target || next > limits.max_json_bytes)) break;
    if (next > limits.max_json_bytes) {
      throw error({
        kind: "hard_limit",
        path: items[i].change.path,
        bytes: next,
        limit: limits.max_json_bytes
      });
    }
    chunk.push(items[i]);
    bytes = next;
    if (bytes > target) break;
  }
  return chunk;
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
            // R15-2: build the chunk straight into `remaining`, skipping
            // every path the server has already refused for these exact
            // bytes. Doing it here (not at each call site) is the point:
            // `rejected` is consulted here for the same reason, so no send
            // path can forget the rule.
            const chunk: T[] = [];
            let bytes = emptyBytes;
            let next = offset;
            while (next < remaining.length && chunk.length < limits.max_changes) {
              const item = remaining[next];
              if (this.isQuarantined(item)) {
                next += 1;
                continue;
              }
              const candidate = bytes + weight(item) + (chunk.length ? 1 : 0);
              if (
                chunk.length &&
                (candidate > this.target || candidate > limits.max_json_bytes)
              ) {
                break;
              }
              if (candidate > limits.max_json_bytes) {
                throw this.makeError({
                  kind: "hard_limit",
                  path: item.change.path,
                  bytes: candidate,
                  limit: limits.max_json_bytes
                });
              }
              chunk.push(item);
              bytes = candidate;
              next += 1;
              if (bytes > this.target) break;
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

  private isQuarantined(item: PushItem): boolean {
    const entry = this.quarantined.get(item.change.path);
    if (!entry) return false;
    if (entry.until <= this.now()) {
      this.quarantined.delete(item.change.path);
      return false;
    }
    // The cooldown only covers the CONTENT the server refused. An edited
    // file has a new fingerprint and is worth a retry immediately.
    return entry.fingerprint === item.fingerprint;
  }
}
