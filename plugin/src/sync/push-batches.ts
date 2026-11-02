import { ApiError } from "../api/client";
import { pushBodyBytes, pushChangeBytes } from "../api/push-request";
import type { PushChange, PushResponse } from "./types";
import type { PushFailure } from "./push-errors";
import {
  DEFAULT_PUSH_TARGET_BYTES,
  MAX_413_FAILURES,
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
    }
    for (const [path, entry] of this.rejected) {
      if (entry.until <= this.now()) this.rejected.delete(path);
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
            }
            const chunk = takePrefix(
              remaining,
              offset,
              callbacks.deviceName,
              this.target,
              limits,
              weight,
              this.makeError
            );
            const bytes = pushBodyBytes(
              chunk.map((item) => item.change),
              callbacks.deviceName
            );
            if (chunk.length === 0 || bytes > limits.max_json_bytes) {
              throw new Error("invalid_push_partition");
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
              offset += chunk.length;
              continue;
            }
            assertActive();
            await callbacks.onAccepted(chunk, response);
            assertActive();
            offset += chunk.length;
          }
        };
        await sendPacked(items);
      }
    };
  }
}
