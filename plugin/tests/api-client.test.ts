import { requestUrl } from "obsidian";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiClient, __test } from "../src/api/client";

const requestUrlMock = vi.mocked(requestUrl);

function mockResponse(text: string, status = 200) {
  requestUrlMock.mockResolvedValue({
    status,
    headers: {},
    arrayBuffer: new ArrayBuffer(0),
    json: JSON.parse(text),
    text
  });
}

describe("ApiClient helpers", () => {
  beforeEach(() => {
    requestUrlMock.mockReset();
  });

  it("sends deployment key, plugin user agent, auth header, and JSON body", async () => {
    mockResponse(
      '{"token":"tok","user_id":"u1","username":"alice","is_admin":false}'
    );
    const client = new ApiClient({
      serverUrl: "https://sync.example.com/base",
      deploymentKey: "k_abc",
      token: "existing",
      pluginVersion: "0.1.0"
    });

    const response = await client.login("alice", "secret", "Laptop");

    expect(response.token).toBe("tok");
    expect(requestUrlMock).toHaveBeenCalledWith({
      url: "https://sync.example.com/base/api/auth/login",
      method: "POST",
      headers: {
        "User-Agent": "PKVSync-Plugin/0.1.0",
        "X-PKVSync-Deployment-Key": "k_abc",
        "Content-Type": "application/json"
      },
      body: JSON.stringify({
        username: "alice",
        password: "secret",
        device_name: "Laptop"
      }),
      throw: false
    });

    mockResponse(
      '{"user_id":"u1","username":"alice","is_admin":false,"vaults":[]}'
    );
    await client.me();
    expect(requestUrlMock).toHaveBeenLastCalledWith(
      expect.objectContaining({
        headers: expect.objectContaining({
          Authorization: "Bearer existing"
        })
      })
    );
  });

  it("parses structured error", () => {
    expect(
      __test.tryParseError('{"error":{"code":"bad","message":"No"}}', 400)
    ).toEqual({ code: "bad", message: "No" });
  });

  it("creates vaults with auth", async () => {
    mockResponse(
      '{"id":"v1","user_id":"u1","name":"main","created_at":1,"last_sync_at":null,"size_bytes":0,"file_count":0}',
      201
    );
    const client = new ApiClient({
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_abc",
      token: "tok",
      pluginVersion: "0.1.0"
    });

    const vault = await client.createVault("main");

    expect(vault.id).toBe("v1");
    expect(requestUrlMock).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "https://sync.example.com/api/vaults",
        method: "POST",
        headers: expect.objectContaining({
          Authorization: "Bearer tok",
          "Content-Type": "application/json"
        }),
        body: JSON.stringify({ name: "main" })
      })
    );
  });

  it("falls back for invalid json", () => {
    expect(__test.tryParseError("nope", 404)).toEqual({
      code: "http_404",
      message: "HTTP 404"
    });
  });
});
