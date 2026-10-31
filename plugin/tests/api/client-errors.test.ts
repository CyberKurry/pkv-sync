import { requestUrl } from "obsidian";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiClient, ApiError, retryAfterAt } from "../../src/api/client";

const requestUrlMock = vi.mocked(requestUrl);

function client(): ApiClient {
  return new ApiClient({
    serverUrl: "https://sync.example.com",
    deploymentKey: "k_abc",
    pluginVersion: "1.7.1"
  });
}

function mockError(status: number, headers: Record<string, string>, text = "") {
  requestUrlMock.mockResolvedValue({
    status,
    headers,
    arrayBuffer: new ArrayBuffer(0),
    json: undefined,
    text
  });
}

async function rejection(promise: Promise<unknown>): Promise<ApiError> {
  return promise.then(
    () => {
      throw new Error("expected the request to fail");
    },
    (reason: unknown) => reason as ApiError
  );
}

describe("retryAfterAt", () => {
  const now = 1_700_000_000_000;

  it("reads delay seconds", () => {
    expect(retryAfterAt({ "retry-after": "120" }, now)).toBe(now + 120_000);
  });

  it("reads an HTTP date", () => {
    const date = new Date(now + 90_000).toUTCString();
    expect(retryAfterAt({ "Retry-After": date }, now)).toBe(now + 90_000);
  });

  it("matches the header name case-insensitively", () => {
    expect(retryAfterAt({ "RETRY-AFTER": " 5 " }, now)).toBe(now + 5_000);
  });

  it.each(["", "   ", "soon", "-5", "1.5", "1e3"])(
    "ignores the invalid value %j",
    (value) => {
      expect(retryAfterAt({ "retry-after": value }, now)).toBeUndefined();
    }
  );

  it("ignores an already expired HTTP date", () => {
    expect(
      retryAfterAt({ "retry-after": new Date(now - 1000).toUTCString() }, now)
    ).toBeUndefined();
  });

  it("ignores zero seconds", () => {
    expect(retryAfterAt({ "retry-after": "0" }, now)).toBeUndefined();
  });

  it("returns undefined when the header is absent", () => {
    expect(retryAfterAt({}, now)).toBeUndefined();
  });
});

describe("ApiError retry metadata", () => {
  beforeEach(() => {
    requestUrlMock.mockReset();
  });

  it("keeps the three-argument constructor compatible", () => {
    const error = new ApiError(413, "http_413", "HTTP 413");
    expect(error.status).toBe(413);
    expect(error.code).toBe("http_413");
    expect(error.message).toBe("HTTP 413");
    expect(error.retryAfterAt).toBeUndefined();
  });

  it("exposes the retry deadline from an HTML 413 response", async () => {
    mockError(413, { "retry-after": "60" }, "<html>too large</html>");
    const before = Date.now();
    const error = await rejection(client().config());
    expect(error.status).toBe(413);
    expect(error.code).toBe("http_413");
    expect(error.retryAfterAt).toBeGreaterThanOrEqual(before + 60_000);
    expect(error.retryAfterAt).toBeLessThanOrEqual(Date.now() + 60_000);
  });

  it("leaves the deadline unset when the header is not usable", async () => {
    mockError(413, { "retry-after": "never" });
    const error = await rejection(client().config());
    expect(error.retryAfterAt).toBeUndefined();
  });
});
