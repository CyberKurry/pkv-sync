import { requestUrl } from "obsidian";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiClient, ApiError, tryParseError } from "../src/api/client";
import type { VaultSettings } from "../src/api/types";

const requestUrlMock = vi.mocked(requestUrl);

function mockResponse(text: string, status = 200) {
  requestUrlMock.mockResolvedValue({
    status,
    headers: {},
    arrayBuffer: new ArrayBuffer(0),
    json: text.length === 0 ? undefined : JSON.parse(text),
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

    const response = await client.login("alice", "secret", "dev_123", "Laptop");

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
        device_id: "dev_123",
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
      tryParseError('{"error":{"code":"bad","message":"No"}}', 400)
    ).toEqual({ code: "bad", message: "No" });
  });

  it("falls back when structured error fields are not strings", () => {
    expect(
      tryParseError('{"error":{"code":123,"message":{"nested":true}}}', 418)
    ).toEqual({ code: "http_418", message: "HTTP 418" });
  });

  it("preserves setup_required server errors as ApiError code", async () => {
    mockResponse(
      '{"error":{"code":"setup_required","message":"Initial setup required"}}',
      403
    );
    const client = new ApiClient({
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_abc",
      pluginVersion: "0.1.0"
    });

    await expect(client.config()).rejects.toMatchObject({
      status: 403,
      code: "setup_required",
      message: "Initial setup required"
    } satisfies Partial<ApiError>);
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
    expect(tryParseError("nope", 404)).toEqual({
      code: "http_404",
      message: "HTTP 404"
    });
  });

  it("deleteVault calls DELETE /api/vaults/:id with auth", async () => {
    requestUrlMock.mockResolvedValueOnce({ status: 204, headers: {}, text: "", arrayBuffer: new ArrayBuffer(0), json: null });
    const client = new ApiClient({
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_test",
      token: "tok_abc",
      pluginVersion: "0.2.0"
    });
    await client.deleteVault("vault-123");
    expect(requestUrlMock).toHaveBeenCalledWith(
      expect.objectContaining({
        method: "DELETE",
        url: "https://sync.example.com/api/vaults/vault-123",
        headers: expect.objectContaining({
          Authorization: "Bearer tok_abc"
        })
      })
    );
  });

  it("gets vault settings with auth and URL-encodes the vault id", async () => {
    mockResponse('{"extra_sync_globs":[".obsidian/themes/**"]}');
    const client = new ApiClient({
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_test",
      token: "tok_abc",
      pluginVersion: "0.2.0"
    });

    const settings: VaultSettings = await client.getVaultSettings("vault/with space");

    expect(settings).toEqual({ extra_sync_globs: [".obsidian/themes/**"] });
    expect(requestUrlMock).toHaveBeenCalledWith(
      expect.objectContaining({
        method: "GET",
        url: "https://sync.example.com/api/vaults/vault%2Fwith%20space/settings",
        headers: expect.objectContaining({
          Authorization: "Bearer tok_abc"
        })
      })
    );
  });

  it("puts vault settings with auth and accepts no response body", async () => {
    mockResponse("", 204);
    const client = new ApiClient({
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_test",
      token: "tok_abc",
      pluginVersion: "0.2.0"
    });

    const result = await client.putVaultSettings("vault-123", {
      extra_sync_globs: [".claude/agents/**"]
    });

    expect(result).toBeUndefined();
    expect(requestUrlMock).toHaveBeenCalledWith(
      expect.objectContaining({
        method: "PUT",
        url: "https://sync.example.com/api/vaults/vault-123/settings",
        headers: expect.objectContaining({
          Authorization: "Bearer tok_abc",
          "Content-Type": "application/json"
        }),
        body: JSON.stringify({ extra_sync_globs: [".claude/agents/**"] })
      })
    );
  });
});
