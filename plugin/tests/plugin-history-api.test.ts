import { requestUrl } from "obsidian";
import { afterEach, describe, expect, it, vi } from "vitest";
import PKVSyncPlugin from "../src/main";
import type { PKVSyncSettings } from "../src/settings";

type HistoryApiHarness = {
  settings: PKVSyncSettings;
  manifest: { version: string };
  app?: {
    vault: {
      adapter: {
        read(path: string): Promise<string>;
        write(path: string, data: string): Promise<void>;
        remove(path: string): Promise<void>;
        mkdir(path: string): Promise<void>;
      };
      configDir: string;
    };
  };
  client: unknown | null;
  historyClient: unknown | null;
  updateClient?: unknown | null;
  api(): unknown;
  historyApi(): {
    commits(vaultId: string): Promise<unknown>;
  };
  updateService(): {
    checkOnce(source: "server" | "github"): Promise<unknown>;
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
    plugin.manifest = { version: "1.0.5" };
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

  it("reuses update service until plugin file location changes", async () => {
    requestUrlMock.mockResolvedValue({
      status: 200,
      headers: { "content-type": "application/json" },
      arrayBuffer: new ArrayBuffer(0),
      json: {
        version: "1.0.5",
        main_js_url: "https://sync.example.com/main.js",
        main_js_sha256: "0".repeat(64),
        manifest_json_url: "https://sync.example.com/manifest.json"
      },
      text: JSON.stringify({
        version: "1.0.5",
        main_js_url: "https://sync.example.com/main.js",
        main_js_sha256: "0".repeat(64),
        manifest_json_url: "https://sync.example.com/manifest.json"
      })
    });
    const adapter = {
      read: async (_path: string) => "",
      write: async (_path: string, _data: string) => undefined,
      remove: async (_path: string) => undefined,
      mkdir: async (_path: string) => undefined
    };
    const plugin = Object.create(PKVSyncPlugin.prototype) as HistoryApiHarness;
    plugin.client = null;
    plugin.updateClient = null;
    plugin.manifest = { version: "1.0.3" };
    plugin.settings = {
      serverUrl: "https://one.example.com",
      deploymentKey: "k_one",
      token: "tok_one"
    } as PKVSyncSettings;
    plugin.app = {
      vault: {
        adapter,
        configDir: ".obsidian"
      }
    };

    const first = plugin.updateService();
    await first.checkOnce("server");
    plugin.settings.serverUrl = "https://two.example.com";
    plugin.settings.deploymentKey = "k_two";
    plugin.settings.token = "tok_two";
    const second = plugin.updateService();
    await second.checkOnce("server");
    plugin.app.vault.configDir = ".config";
    const third = plugin.updateService();

    expect(second).toBe(first);
    expect(third).not.toBe(first);
    expect(requestUrlMock).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({
        url: "https://one.example.com/api/plugin-manifest",
        headers: expect.objectContaining({
          Authorization: "Bearer tok_one",
          "X-PKVSync-Deployment-Key": "k_one"
        })
      })
    );
    expect(requestUrlMock).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({
        url: "https://two.example.com/api/plugin-manifest",
        headers: expect.objectContaining({
          Authorization: "Bearer tok_two",
          "X-PKVSync-Deployment-Key": "k_two"
        })
      })
    );
  });
});
