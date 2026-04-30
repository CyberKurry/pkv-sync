import { requestUrl } from "obsidian";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiClient } from "../src/api/client";
import { SyncApi } from "../src/api/sync-client";

const requestUrlMock = vi.mocked(requestUrl);

function client(): ApiClient {
  return new ApiClient({
    serverUrl: "https://sync.example.com",
    deploymentKey: "k_abc",
    token: "tok",
    pluginVersion: "0.1.0"
  });
}

describe("SyncApi", () => {
  beforeEach(() => {
    requestUrlMock.mockReset();
  });

  it("requests vault state with head_since", async () => {
    requestUrlMock.mockResolvedValue({
      status: 200,
      headers: {},
      arrayBuffer: new ArrayBuffer(0),
      json: { current_head: "c1", changed_since: true },
      text: '{"current_head":"c1","changed_since":true}'
    });

    const response = await new SyncApi(client()).state("v1", "c0");

    expect(response.changed_since).toBe(true);
    expect(requestUrlMock).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "https://sync.example.com/api/vaults/v1/state?head_since=c0",
        method: "GET",
        headers: expect.objectContaining({
          Authorization: "Bearer tok"
        })
      })
    );
  });

  it("uploads raw blob bytes with content hash and accepts empty 201", async () => {
    const bytes = new Uint8Array([1, 2, 3]).buffer;
    requestUrlMock.mockResolvedValue({
      status: 201,
      headers: {},
      arrayBuffer: new ArrayBuffer(0),
      json: {},
      text: ""
    });

    await new SyncApi(client()).uploadBlob("v1", "abc", bytes);

    expect(requestUrlMock).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "https://sync.example.com/api/vaults/v1/upload/blob",
        method: "POST",
        body: bytes,
        headers: expect.objectContaining({
          "content-hash": "abc"
        })
      })
    );
  });

  it("downloads blob bytes from octet-stream responses", async () => {
    const bytes = new Uint8Array([4, 5, 6]).buffer;
    requestUrlMock.mockResolvedValue({
      status: 200,
      headers: { "content-type": "application/octet-stream" },
      arrayBuffer: bytes,
      json: {},
      text: ""
    });

    await expect(new SyncApi(client()).downloadBlob("v1", "def")).resolves.toBe(
      bytes
    );
  });

  it("downloads full text file content for non-inline pull entries", async () => {
    requestUrlMock.mockResolvedValue({
      status: 200,
      headers: { "content-type": "text/plain; charset=utf-8" },
      arrayBuffer: new ArrayBuffer(0),
      json: {},
      text: "large text"
    });

    await expect(
      new SyncApi(client()).downloadTextFile("v1", "folder/a b.md", "c1")
    ).resolves.toBe("large text");
    expect(requestUrlMock).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "https://sync.example.com/api/vaults/v1/files/folder/a%20b.md?at=c1"
      })
    );
  });
});
