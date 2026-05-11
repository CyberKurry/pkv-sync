import { requestUrl } from "obsidian";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiClient } from "../../src/api/client";
import { HistoryApi } from "../../src/api/history-client";

const requestUrlMock = vi.mocked(requestUrl);

function client(): ApiClient {
  return new ApiClient({
    serverUrl: "https://sync.example.com",
    deploymentKey: "k_abc",
    token: "tok",
    pluginVersion: "0.1.0"
  });
}

function mockJson(text: string): void {
  requestUrlMock.mockResolvedValue({
    status: 200,
    headers: { "content-type": "application/json" },
    arrayBuffer: new ArrayBuffer(0),
    json: JSON.parse(text),
    text
  });
}

describe("HistoryApi", () => {
  beforeEach(() => {
    requestUrlMock.mockReset();
  });

  it("requests file history with encoded path and clamped limit", async () => {
    mockJson("[]");

    await new HistoryApi(client()).fileHistory("v1", "folder/a b.md", 500);

    expect(requestUrlMock).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "https://sync.example.com/api/vaults/v1/history?path=folder%2Fa%20b.md&limit=200",
        method: "GET",
        headers: expect.objectContaining({ Authorization: "Bearer tok" })
      })
    );
  });

  it("requests unified diff with from/to/path query", async () => {
    mockJson(
      '{"from":"c1","to":"c2","path":"note.md","binary":false,"truncated":false,"patch":"@@\\n+hello"}'
    );

    const diff = await new HistoryApi(client()).diff("v1", {
      from: "c1",
      to: "c2",
      path: "note.md"
    });

    expect(diff.patch).toContain("+hello");
    expect(requestUrlMock).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "https://sync.example.com/api/vaults/v1/diff?to=c2&path=note.md&from=c1"
      })
    );
  });

  it("reads text and binary historical file content", async () => {
    requestUrlMock.mockResolvedValueOnce({
      status: 200,
      headers: { "content-type": "text/plain; charset=utf-8" },
      arrayBuffer: new ArrayBuffer(0),
      json: {},
      text: "hello"
    });
    await expect(
      new HistoryApi(client()).readFileAt("v1", "note.md", "c1")
    ).resolves.toEqual({ kind: "text", text: "hello" });

    const bytes = new Uint8Array([1, 2, 3]).buffer;
    requestUrlMock.mockResolvedValueOnce({
      status: 200,
      headers: { "content-type": "application/octet-stream" },
      arrayBuffer: bytes,
      json: {},
      text: ""
    });
    await expect(
      new HistoryApi(client()).readFileAt("v1", "image.png", "c1")
    ).resolves.toEqual({ kind: "binary", bytes });
  });
});
