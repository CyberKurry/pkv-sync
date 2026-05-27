import { requestUrl } from "obsidian";
import { afterEach, describe, expect, it, vi } from "vitest";
import PKVSyncPlugin from "../src/main";
import type { PKVSyncSettings } from "../src/settings";

type HistoryApiHarness = {
  settings: PKVSyncSettings;
  manifest: { version: string };
  client: unknown | null;
  historyClient: unknown | null;
  api(): unknown;
  historyApi(): {
    commits(vaultId: string): Promise<unknown>;
  };
};

const requestUrlMock = vi.mocked(requestUrl);

describe("PKVSyncPlugin history API cache", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    requestUrlMock.mockReset();
  });

  it("reuses the HistoryApi wrapper while refreshing ApiClient settings", async () => {
    requestUrlMock.mockResolvedValue({
      status: 200,
      headers: { "content-type": "application/json" },
      arrayBuffer: new ArrayBuffer(0),
      json: [],
      text: "[]"
    });
    const plugin = Object.create(PKVSyncPlugin.prototype) as HistoryApiHarness;
    plugin.client = null;
    plugin.historyClient = null;
    plugin.manifest = { version: "1.0.4" };
    plugin.settings = {
      serverUrl: "https://one.example.com",
      deploymentKey: "k_one",
      token: "tok_one"
    } as PKVSyncSettings;

    const first = plugin.historyApi();
    await first.commits("vault-a");
    plugin.settings.serverUrl = "https://two.example.com";
    plugin.settings.deploymentKey = "k_two";
    plugin.settings.token = "tok_two";
    const second = plugin.historyApi();
    await second.commits("vault-b");

    expect(second).toBe(first);
    expect(requestUrlMock).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        url: "https://one.example.com/api/vaults/vault-a/commits?limit=50",
        headers: expect.objectContaining({
          Authorization: "Bearer tok_one",
          "X-PKVSync-Deployment-Key": "k_one"
        })
      })
    );
    expect(requestUrlMock).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        url: "https://two.example.com/api/vaults/vault-b/commits?limit=50",
        headers: expect.objectContaining({
          Authorization: "Bearer tok_two",
          "X-PKVSync-Deployment-Key": "k_two"
        })
      })
    );
  });
});
