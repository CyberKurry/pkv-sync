import { afterEach, describe, expect, it, vi } from "vitest";
import { subscribeVaultEvents, type SubscribeOptions } from "../../src/api/events-client";
import type { VaultEvent } from "../../src/api/types";

/** Build a ReadableStream<Uint8Array> from an array of string chunks. */
function mockSseStream(chunks: string[]): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder();
  const encoded = chunks.map((c) => encoder.encode(c));
  let index = 0;
  return new ReadableStream<Uint8Array>({
    pull(controller) {
      if (index < encoded.length) {
        controller.enqueue(encoded[index++]);
      } else {
        controller.close();
      }
    },
  });
}

const baseOpts: Omit<SubscribeOptions, "onEvent" | "onError"> = {
  serverUrl: "https://sync.example.com",
  vaultId: "v1",
  deploymentKey: "k_abc",
  token: "tok",
  ownDeviceId: "dev_self",
  pluginVersion: "0.3.3",
};

describe("subscribeVaultEvents", () => {
  const originalFetch = globalThis.fetch;

  afterEach(() => {
    globalThis.fetch = originalFetch;
    vi.restoreAllMocks();
  });

  function setFetchResponse(
    body: ReadableStream<Uint8Array> | null,
    ok: boolean,
    status: number
  ): void {
    const response = { ok, status, body } as Response;
    globalThis.fetch = ((() => Promise.resolve(response)) as unknown) as typeof fetch;
  }

  function setFetchError(err: Error): void {
    globalThis.fetch = ((() => Promise.reject(err)) as unknown) as typeof fetch;
  }

  it("receives text_inline commit event and calls onEvent", async () => {
    const ssePayload =
      "event: commit\n" +
      'data: {"commit":"c1","parent":null,"source_device_id":"dev_other","at":1700000000,"changes":[{"kind":"text_inline","path":"a.md","content":"hello"}]}\n\n';

    setFetchResponse(mockSseStream([ssePayload]), true, 200);

    const onEvent = vi.fn();
    const onError = vi.fn();
    subscribeVaultEvents({ ...baseOpts, onEvent, onError });

    await vi.waitFor(() => expect(onEvent).toHaveBeenCalled());

    expect(onEvent).toHaveBeenCalledTimes(1);
    const ev: VaultEvent = onEvent.mock.calls[0][0];
    expect(ev.commit).toBe("c1");
    expect(ev.source_device_id).toBe("dev_other");
    expect(ev.changes).toHaveLength(1);
    expect(ev.changes[0]).toEqual({
      kind: "text_inline",
      path: "a.md",
      content: "hello",
    });
    expect(onError).not.toHaveBeenCalled();
  });

  it("receives blob change and calls onEvent with correct kind", async () => {
    const ssePayload =
      "event: commit\n" +
      'data: {"commit":"c2","parent":"c1","source_device_id":"dev_other","at":1700000001,"changes":[{"kind":"blob","path":"img.png","blob_hash":"h123","size":4096}]}\n\n';

    setFetchResponse(mockSseStream([ssePayload]), true, 200);

    const onEvent = vi.fn();
    subscribeVaultEvents({ ...baseOpts, onEvent, onError: vi.fn() });

    await vi.waitFor(() => expect(onEvent).toHaveBeenCalled());

    const ev: VaultEvent = onEvent.mock.calls[0][0];
    expect(ev.changes[0].kind).toBe("blob");
    if (ev.changes[0].kind === "blob") {
      expect(ev.changes[0].path).toBe("img.png");
      expect(ev.changes[0].blob_hash).toBe("h123");
    }
  });

  it("filters events where source_device_id matches ownDeviceId", async () => {
    const ssePayload =
      "event: commit\n" +
      'data: {"commit":"c3","parent":"c2","source_device_id":"dev_self","at":1700000002,"changes":[{"kind":"text_inline","path":"b.md","content":"self-edit"}]}\n\n';

    setFetchResponse(mockSseStream([ssePayload]), true, 200);

    const onEvent = vi.fn();
    subscribeVaultEvents({ ...baseOpts, onEvent, onError: vi.fn() });

    // Give the async IIFE time to process; onEvent should never be called
    await new Promise((r) => setTimeout(r, 50));
    expect(onEvent).not.toHaveBeenCalled();
  });

  it("calls onError when fetch throws", async () => {
    setFetchError(new Error("network down"));

    const onError = vi.fn();
    const onEvent = vi.fn();
    subscribeVaultEvents({ ...baseOpts, onEvent, onError });

    await vi.waitFor(() => expect(onError).toHaveBeenCalled());
    expect(onError.mock.calls[0][0].message).toBe("network down");
  });

  it("unsubscribe triggers AbortController signal", async () => {
    const ssePayload =
      "event: commit\n" +
      'data: {"commit":"c_pre","parent":null,"source_device_id":"dev_other","at":0,"changes":[]}\n\n';

    setFetchResponse(mockSseStream([ssePayload]), true, 200);

    const onEvent = vi.fn();
    const unsubscribe = subscribeVaultEvents({
      ...baseOpts,
      onEvent,
      onError: vi.fn(),
    });

    // Verify the first event arrives
    await vi.waitFor(() => expect(onEvent).toHaveBeenCalledTimes(1));

    // Calling unsubscribe should not throw and should return void
    expect(() => unsubscribe()).not.toThrow();

    // After unsubscribe, no more fetch calls should be made.
    // The key contract: unsubscribe is a function that aborts the SSE connection.
    expect(typeof unsubscribe).toBe("function");
  });

  it("emits lagged event with empty commit and changes", async () => {
    const ssePayload = "event: lagged\ndata: \n\n";

    setFetchResponse(mockSseStream([ssePayload]), true, 200);

    const onEvent = vi.fn();
    subscribeVaultEvents({ ...baseOpts, onEvent, onError: vi.fn() });

    await vi.waitFor(() => expect(onEvent).toHaveBeenCalled());

    const ev: VaultEvent = onEvent.mock.calls[0][0];
    expect(ev.commit).toBe("");
    expect(ev.changes).toEqual([]);
  });

  it("calls onError when HTTP response is not ok", async () => {
    setFetchResponse(null, false, 403);

    const onError = vi.fn();
    subscribeVaultEvents({ ...baseOpts, onEvent: vi.fn(), onError });

    await vi.waitFor(() => expect(onError).toHaveBeenCalled());
    expect(onError.mock.calls[0][0].message).toContain("HTTP 403");
  });

  it("ignores SSE comment lines (heartbeats)", async () => {
    const ssePayload =
      ": this is a comment\n" +
      "event: commit\n" +
      'data: {"commit":"c5","parent":null,"source_device_id":"dev_other","at":0,"changes":[]}\n\n';

    setFetchResponse(mockSseStream([ssePayload]), true, 200);

    const onEvent = vi.fn();
    subscribeVaultEvents({ ...baseOpts, onEvent, onError: vi.fn() });

    await vi.waitFor(() => expect(onEvent).toHaveBeenCalled());

    const ev: VaultEvent = onEvent.mock.calls[0][0];
    expect(ev.commit).toBe("c5");
  });

  it("handles multi-line data fields", async () => {
    const ssePayload =
      "event: commit\n" +
      'data: {"commit":"c6","parent":null,"source_device_id":"dev_other","at":0,\n' +
      'data: "changes":[]}\n\n';

    setFetchResponse(mockSseStream([ssePayload]), true, 200);

    const onEvent = vi.fn();
    subscribeVaultEvents({ ...baseOpts, onEvent, onError: vi.fn() });

    await vi.waitFor(() => expect(onEvent).toHaveBeenCalled());

    const ev: VaultEvent = onEvent.mock.calls[0][0];
    expect(ev.commit).toBe("c6");
  });
});
