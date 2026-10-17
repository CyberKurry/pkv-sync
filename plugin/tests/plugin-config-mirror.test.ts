import { describe, expect, it, vi } from "vitest";
import PKVSyncPlugin from "../src/main";

/**
 * S6 (client half): every server-config fetch must mirror the server's
 * supported_text_extensions into local settings. A stale local set makes
 * the client push text files at paths the server has reconfigured, and
 * the wrong_file_kind 400 then wedges the whole batch — every sync pass
 * re-sends the same rejected change forever.
 */
type Harness = {
  settings: {
    serverUrl: string;
    deploymentKey: string;
    debounceMs: number;
    textExtensions: string[];
  };
  serverCapabilities: unknown;
  dataStore: {
    update(updater: (data: unknown) => unknown): Promise<void>;
  };
  api(): { config(): Promise<unknown> };
  updateStatus(): void;
  rebuildSyncEngine(): void;
  refreshServerCapabilities(allowRebuild?: boolean): Promise<void>;
};

function makePlugin(configResponse: {
  supported_text_extensions?: string[];
  push_debounce_ms?: number;
}): { plugin: Harness; saved: unknown[]; getRebuilt(): number } {
  const saved: unknown[] = [];
  let rebuilt = 0;
  const plugin = Object.create(PKVSyncPlugin.prototype) as Harness;
  plugin.settings = {
    serverUrl: "https://sync.example.com",
    deploymentKey: "k_x",
    debounceMs: 250,
    textExtensions: ["md", "txt", "css"]
  };
  plugin.dataStore = {
    update: async (updater) => {
      saved.push(updater({}));
    }
  };
  plugin.api = vi.fn(() => ({
    config: vi.fn().mockResolvedValue({
      server_name: "s",
      version: "1.0.0",
      registration: "open",
      max_file_size: 1000,
      supported_text_extensions: ["md"],
      capabilities: { history: true, diff: true, sse: true },
      ...configResponse
    })
  }));
  plugin.updateStatus = vi.fn();
  plugin.rebuildSyncEngine = () => {
    rebuilt += 1;
  };
  return { plugin, saved, getRebuilt: () => rebuilt };
}

describe("refreshServerCapabilities mirrors server config (S6)", () => {
  it("adopts the server's text extension set when it differs", async () => {
    const { plugin, saved, getRebuilt } = makePlugin({
      supported_text_extensions: ["md", "canvas", "json"],
      push_debounce_ms: 250
    });

    await plugin.refreshServerCapabilities();
    expect(plugin.settings.textExtensions).toEqual(["md", "canvas", "json"]);
    expect(saved).toHaveLength(1);
    expect(getRebuilt()).toBe(1);
  });

  it("does not rebuild when the server only reorders the extension list (W6)", async () => {
    const { plugin, saved, getRebuilt } = makePlugin({
      supported_text_extensions: ["css", "md", "txt"],
      push_debounce_ms: 250
    });

    await plugin.refreshServerCapabilities();

    expect(plugin.settings.textExtensions).toEqual(["md", "txt", "css"]);
    expect(saved).toHaveLength(0);
    expect(getRebuilt()).toBe(0);
  });

  it("does not rebuild when the caller forbids it (W7 migration path)", async () => {
    const { plugin, saved, getRebuilt } = makePlugin({
      supported_text_extensions: ["md", "canvas"],
      push_debounce_ms: 250
    });

    await plugin.refreshServerCapabilities(false);

    // The mirror still persists the new set...
    expect(plugin.settings.textExtensions).toEqual(["md", "canvas"]);
    expect(saved).toHaveLength(1);
    // ...but must not start an engine before the migrated index is saved.
    expect(getRebuilt()).toBe(0);
  });

  it("does not rewrite settings when the set already matches", async () => {
    const { plugin, saved, getRebuilt } = makePlugin({
      supported_text_extensions: ["md", "txt", "css"],
      push_debounce_ms: 250
    });

    await plugin.refreshServerCapabilities();

    expect(plugin.settings.textExtensions).toEqual(["md", "txt", "css"]);
    expect(saved).toHaveLength(0);
    expect(getRebuilt()).toBe(0);
  });
});
