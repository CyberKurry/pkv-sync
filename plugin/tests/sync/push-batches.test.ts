import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { pushBodyBytes } from "../../src/api/push-request";
import { en } from "../../src/i18n/en";
import { zh } from "../../src/i18n/zh";
import {
  PushBatcher,
  type PushItem
} from "../../src/sync/push-batches";
import { PushBatchError, type PushFailure } from "../../src/sync/push-errors";
import {
  LEGACY_PUSH_LIMITS,
  MAX_413_FAILURES,
  SINGLETON_RETRY_MS,
  type PushLimits
} from "../../src/sync/push-limits";
import type { PushResponse } from "../../src/sync/types";

const RESPONSE: PushResponse = { new_commit: "c1", files_changed: 1 };

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function textItems(count: number, bytes: number): PushItem[] {
  return Array.from({ length: count }, (_, index) => ({
    change: { kind: "text" as const, path: `note-${index}.md`, content: "a".repeat(bytes) },
    fingerprint: `${index}:${bytes}`
  }));
}

function failure(promise: Promise<void>): Promise<PushBatchError> {
  return promise.then(
    () => {
      throw new Error("expected the push to fail");
    },
    (reason: unknown) => reason as PushBatchError
  );
}

function batcher(now?: () => number): PushBatcher {
  return new PushBatcher(
    (detail) => new PushBatchError(detail, en),
    now
  );
}

function rejection(status: number, headers: Record<string, string> = {}): ApiError {
  return new ApiError(status, `http_${status}`, `HTTP ${status}`, undefined);
}

function itemOfBodyBytes(total: number, deviceName = "d"): PushItem {
  const base = pushBodyBytes(
    [{ kind: "text", path: "p.md", content: "" }],
    deviceName
  );
  return {
    change: { kind: "text", path: "p.md", content: "a".repeat(total - base) },
    fingerprint: `p:${total}:${deviceName}`
  };
}

function bodyOf(items: readonly PushItem[], deviceName = "d"): number {
  return pushBodyBytes(items.map((item) => item.change), deviceName);
}

describe("PushBatcher batching", () => {
  it("awaits each accepted sub-batch before sending its successor", async () => {
    const events: string[] = [];
    const release = deferred<void>();
    const items = textItems(4, 300_000);
    const callbacks = {
      deviceName: "d",
      send: vi.fn(async () => {
        events.push("send");
        return { new_commit: "c1", files_changed: 1 };
      }),
      onAccepted: vi.fn(async () => {
        events.push("ack");
        await release.promise;
      })
    };
    const run = batcher().beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    const pending = run.send(items, callbacks);
    await vi.waitFor(() => expect(events).toEqual(["send", "ack"]));
    release.resolve();
    await pending;
    expect(events).toEqual(["send", "ack", "send", "ack"]);
  });

  it("splits 1,001 tiny changes into a 1,000-change batch and a remainder", async () => {
    const counts: number[] = [];
    const run = batcher().beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await run.send(textItems(1001, 4), {
      deviceName: "d",
      send: async (items) => {
        counts.push(items.length);
        return RESPONSE;
      },
      onAccepted: async () => undefined
    });
    expect(counts).toEqual([1000, 1]);
  });

  it("allows a single change whose body equals the soft target", async () => {
    const sent: number[] = [];
    const item = itemOfBodyBytes(1_048_576);
    const run = batcher().beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await run.send([item], {
      deviceName: "d",
      send: async (items) => {
        sent.push(bodyOf(items));
        return RESPONSE;
      },
      onAccepted: async () => undefined
    });
    expect(sent).toEqual([1_048_576]);
  });

  it("sends a single change one byte above the soft target", async () => {
    const sent: number[] = [];
    const run = batcher().beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await run.send([itemOfBodyBytes(1_048_577)], {
      deviceName: "d",
      send: async (items) => {
        sent.push(bodyOf(items));
        return RESPONSE;
      },
      onAccepted: async () => undefined
    });
    expect(sent).toEqual([1_048_577]);
  });

  it("sends a single change whose body equals the hard limit", async () => {
    const sent: number[] = [];
    const run = batcher().beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await run.send([itemOfBodyBytes(2_097_152)], {
      deviceName: "d",
      send: async (items) => {
        sent.push(bodyOf(items));
        return RESPONSE;
      },
      onAccepted: async () => undefined
    });
    expect(sent).toEqual([2_097_152]);
  });

  it("sends no request for a single change above the hard limit", async () => {
    const send = vi.fn(async () => RESPONSE);
    const run = batcher().beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await expect(
      run.send([itemOfBodyBytes(2_097_153)], {
        deviceName: "d",
        send,
        onAccepted: async () => undefined
      })
    ).rejects.toMatchObject({ detail: { kind: "hard_limit", limit: 2_097_152 } });
    expect(send).not.toHaveBeenCalled();
  });

  it("sends a 1.5 MiB single file above the target but below the hard limit", async () => {
    const sent: number[] = [];
    const run = batcher().beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await run.send([itemOfBodyBytes(1_572_864)], {
      deviceName: "d",
      send: async (items) => {
        sent.push(bodyOf(items));
        return RESPONSE;
      },
      onAccepted: async () => undefined
    });
    expect(sent).toEqual([1_572_864]);
  });
});

describe("PushBatcher 413 recovery", () => {
  it("shrinks the target repeatedly until a smaller proxy accepts every batch", async () => {
    const proxyLimit = 300_000;
    const sent: number[] = [];
    const run = batcher().beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await run.send(textItems(10, 200_000), {
      deviceName: "d",
      send: async (items) => {
        const bytes = bodyOf(items);
        sent.push(bytes);
        if (bytes > proxyLimit) throw rejection(413);
        return RESPONSE;
      },
      onAccepted: async () => undefined
    });
    expect(sent.length).toBeGreaterThan(1);
    expect(sent.filter((bytes) => bytes > proxyLimit)).toHaveLength(2);
    expect(sent.at(-1)).toBeLessThanOrEqual(proxyLimit);
  });

  it("does not split when the accepted callback throws a 413", async () => {
    const send = vi.fn(async () => RESPONSE);
    const callbacks = {
      deviceName: "d",
      send,
      onAccepted: vi.fn(async () => {
        throw rejection(413);
      })
    };
    const run = batcher().beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await expect(run.send(textItems(4, 300_000), callbacks)).rejects.toMatchObject({
      status: 413
    });
    expect(send).toHaveBeenCalledTimes(1);
  });

  it("propagates a non-413 failure without splitting", async () => {
    const send = vi.fn(async () => {
      throw rejection(500);
    });
    const run = batcher().beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await expect(
      run.send(textItems(4, 300_000), {
        deviceName: "d",
        send,
        onAccepted: async () => undefined
      })
    ).rejects.toMatchObject({ status: 500 });
    expect(send).toHaveBeenCalledTimes(1);
  });

  it("stops sending the remaining half when the run is cancelled", async () => {
    let cancelled = false;
    let sendsAfterCancel = 0;
    const run = batcher().beginRun(LEGACY_PUSH_LIMITS, () => {
      if (cancelled) throw new Error("cancelled");
    });
    await expect(
      run.send(textItems(8, 300_000), {
        deviceName: "d",
        send: async (items) => {
          if (cancelled) sendsAfterCancel += 1;
          if (bodyOf(items) > 700_000) throw rejection(413);
          return RESPONSE;
        },
        onAccepted: async () => {
          cancelled = true;
        }
      })
    ).rejects.toThrow("cancelled");
    expect(sendsAfterCancel).toBe(0);
  });

  it("never sends more requests than the oversized-request failure budget", async () => {
    const attempts: number[] = [];
    const send = vi.fn(async (items: readonly PushItem[]) => {
      attempts.push(items.length);
      throw rejection(413);
    });
    const run = batcher().beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    const error = await failure(
      run.send(textItems(1000, 1), {
        deviceName: "d",
        send,
        onAccepted: async () => undefined
      })
    );
    expect(error).toBeInstanceOf(PushBatchError);
    expect(error.detail.kind).toBe("remote_singleton");
    expect(attempts.length).toBeGreaterThan(1);
    expect(attempts.length).toBeLessThanOrEqual(MAX_413_FAILURES);
  });
});

describe("PushBatcher singleton cooldown", () => {
  it("keeps a rejected file out of mixed batches during its cooldown", async () => {
    const b = batcher(() => 1_000_000);
    const rejected = textItems(1, 600_000)[0];
    const newcomer: PushItem = {
      change: { kind: "text", path: "new.md", content: "b" }, fingerprint: "new"
    };
    const send = vi.fn(async (items: readonly PushItem[]) => {
      if (bodyOf(items) > 512_000) throw rejection(413);
      return RESPONSE;
    });
    const callbacks = { deviceName: "d", send, onAccepted: async () => undefined };
    await expect(b.beginRun(LEGACY_PUSH_LIMITS, () => undefined).send([rejected], callbacks))
      .rejects.toMatchObject({ detail: { kind: "remote_singleton" } });
    send.mockClear();

    await expect(b.beginRun(LEGACY_PUSH_LIMITS, () => undefined).send([rejected, newcomer], callbacks))
      .rejects.toMatchObject({ detail: {
        kind: "remote_singleton", path: rejected.change.path,
        bytes: pushBodyBytes([rejected.change], "d")
      } });
    expect(send).not.toHaveBeenCalled();
  });

  it("does not resend an unchanged rejected single file before the cooldown ends", async () => {
    let now = 1_000_000;
    const b = batcher(() => now);
    const item = textItems(1, 1000)[0];
    const rejecting = vi.fn(async () => {
      throw rejection(413);
    });
    const run = b.beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await expect(
      run.send([item], {
        deviceName: "d",
        send: rejecting,
        onAccepted: async () => undefined
      })
    ).rejects.toMatchObject({ detail: { kind: "remote_singleton" } });
    expect(rejecting).toHaveBeenCalledTimes(1);

    const retry = vi.fn(async () => RESPONSE);
    const skipped = b.beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await expect(
      skipped.send([item], {
        deviceName: "d",
        send: retry,
        onAccepted: async () => undefined
      })
    ).rejects.toMatchObject({ detail: { kind: "remote_singleton" } });
    expect(retry).not.toHaveBeenCalled();

    now += SINGLETON_RETRY_MS;
    const afterCooldown = b.beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await afterCooldown.send([item], {
      deviceName: "d",
      send: async () => RESPONSE,
      onAccepted: async () => undefined
    });
  });

  it("clears the cooldown on a manual oversized probe", async () => {
    const b = batcher(() => 1_000_000);
    const item = textItems(1, 1000)[0];
    const run = b.beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await expect(
      run.send([item], {
        deviceName: "d",
        send: async () => {
          throw rejection(413);
        },
        onAccepted: async () => undefined
      })
    ).rejects.toMatchObject({ detail: { kind: "remote_singleton" } });

    const manual = vi.fn(async () => RESPONSE);
    const probe = b.beginRun(LEGACY_PUSH_LIMITS, () => undefined, true);
    await probe.send([item], {
      deviceName: "d",
      send: manual,
      onAccepted: async () => undefined
    });
    expect(manual).toHaveBeenCalledTimes(1);
  });

  it("clears the cooldown when the advertised limits change", async () => {
    const b = batcher(() => 1_000_000);
    const item = textItems(1, 1000)[0];
    const run = b.beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await expect(
      run.send([item], {
        deviceName: "d",
        send: async () => {
          throw rejection(413);
        },
        onAccepted: async () => undefined
      })
    ).rejects.toMatchObject({ detail: { kind: "remote_singleton" } });

    const changed: PushLimits = { ...LEGACY_PUSH_LIMITS, max_json_bytes: 1_000_000 };
    const send = vi.fn(async () => RESPONSE);
    const next = b.beginRun(changed, () => undefined);
    await next.send([item], {
      deviceName: "d",
      send,
      onAccepted: async () => undefined
    });
    expect(send).toHaveBeenCalledTimes(1);
  });
});

describe("PushBatcher Retry-After", () => {
  it("defers the whole run without splitting when the server sends a deadline", async () => {
    let now = 1_000_000;
    const b = batcher(() => now);
    const send = vi.fn(async () => {
      throw new ApiError(413, "http_413", "HTTP 413", now + 30_000);
    });
    const run = b.beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    const error = await failure(
      run.send(textItems(4, 300_000), {
        deviceName: "d",
        send,
        onAccepted: async () => undefined
      })
    );
    expect(send).toHaveBeenCalledTimes(1);
    expect(error.detail).toEqual({ kind: "retry_after", deadline: 1_030_000 });
    // R7R3-3: the countdown is computed at read time against the REAL
    // clock (the batcher's `now` only drives protocol decisions), so the
    // number is whatever remains when the assertion reads it — the test
    // clock sits at 1s while real time has advanced past the deadline.
    expect(error.message).toMatch(/Retry in \d+s\./);

    const next = vi.fn(async () => RESPONSE);
    expect(() => b.beginRun(LEGACY_PUSH_LIMITS, () => undefined)).toThrow(
      PushBatchError
    );
    expect(next).not.toHaveBeenCalled();
    now += 30_000;
    const resumed = b.beginRun(LEGACY_PUSH_LIMITS, () => undefined);
    await resumed.send(textItems(1, 10), {
      deviceName: "d",
      send: next,
      onAccepted: async () => undefined
    });
    expect(next).toHaveBeenCalledTimes(1);
  });
});

describe("PushBatchError messages", () => {
  const labels = en;
  const zhLabels = zh;

  it("formats each failure kind with the localized labels", () => {
    const cases: Array<[PushFailure, RegExp, RegExp]> = [
      [
        { kind: "hard_limit", path: "a.md", bytes: 10, limit: 5 },
        /Cannot sync a\.md: the serialized request is 10 bytes, above the server's 5-byte inline limit\./,
        /无法同步 a\.md：序列化请求为 10 字节，超过服务端 5 字节的内联上限。/
      ],
      [
        { kind: "remote_singleton", path: "b.md", bytes: 20 },
        /Cannot sync b\.md: even its single-file request \(20 bytes\) was rejected/,
        /无法同步 b\.md：单文件请求（20 字节）仍被服务端或代理判定为过大。/
      ],
      // R7R3-3: the countdown is computed when the message is read, so the
      // deadline must be relative to the read. 9s from now (rounded).
      [
        { kind: "retry_after", deadline: Date.now() + 9_000 },
        /Retry in (8|9)s\./,
        /将在 (8|9) 秒后重试。/
      ],
      [
        { kind: "retry_budget", failures: 16 },
        /Push stopped after 16 oversized-request failures\./,
        /已发生 16 次请求超限，推送已停止。/
      ]
    ];
    for (const [detail, enPattern, zhPattern] of cases) {
      const asEn = new PushBatchError(detail, labels);
      const asZh = new PushBatchError(detail, zhLabels);
      expect(asEn.message).toMatch(enPattern);
      expect(asZh.message).toMatch(zhPattern);
      expect(asEn.name).toBe("PushBatchError");
    }
  });

  it("recomputes the retry_after countdown at read time (R7R3-3)", () => {
    vi.useFakeTimers();
    try {
      const start = Date.now();
      // Construct with a deadline 10s out; read twice with time passing in
      // between. A construction-time countdown would freeze the number.
      const error = new PushBatchError(
        { kind: "retry_after", deadline: start + 10_000 },
        en
      );
      const first = Number(error.message.match(/Retry in (\d+)s/)?.[1] ?? 0);
      expect(first).toBeGreaterThan(0);
      expect(first).toBeLessThanOrEqual(10);

      // Advance most of the window; the second read must show a smaller
      // (accurate) countdown, not the frozen construction-time value.
      vi.setSystemTime(start + 8_000);
      const second = Number(error.message.match(/Retry in (\d+)s/)?.[1] ?? 99);
      expect(second).toBeLessThanOrEqual(2);
    } finally {
      vi.useRealTimers();
    }
  });
});
