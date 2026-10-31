import { notices } from "./mocks/obsidian";
import { requestUrl } from "obsidian";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ServerCapabilities, ServerConfigResponse } from "../src/api/types";
import type { ConnectionIdentity, ConnectionStamp } from "../src/connection-context";
import PKVSyncPlugin from "../src/main";
import { readPluginSettings } from "../src/plugin-data";
import { SerializedPluginDataStore } from "../src/plugin-store";
import { DEFAULT_SETTINGS, type PKVSyncSettings } from "../src/settings";
import { LEGACY_PUSH_LIMITS, type PushLimits } from "../src/sync/push-limits";

interface ConfigPluginHarness {
  settings: PKVSyncSettings;
  manifest: { version: string };
  dataStore: SerializedPluginDataStore;
  serverCapabilities: ServerCapabilities | null;
  serverPushLimits: PushLimits;
  persistenceRecoveryRequired: boolean;
  refreshServerCapabilities(allowRebuild?: boolean): Promise<ServerConfigResponse | null>;
  setConnectionPatch(
    patch: Partial<ConnectionIdentity>,
    expected?: ConnectionStamp
  ): Promise<ConnectionStamp | null>;
  setSelectedVault(vaultId: string, vaultName: string): Promise<boolean>;
  currentPushLimits(): Readonly<PushLimits>;
  updateStatus(): void;
  invalidateSyncEngine(): void;
  rebuildSyncEngine(): void;
}

const identityA: ConnectionIdentity = {
  serverUrl: "https://a.example.test",
  deploymentKey: "key-a",
  token: "test-a",
  userId: "user-a",
  username: "a"
};
const identityB: ConnectionIdentity = {
  serverUrl: "https://b.example.test",
  deploymentKey: "key-b",
  token: "test-b",
  userId: "user-b",
  username: "b"
};

const configA: ServerConfigResponse = {
  server_name: "A",
  version: "1.7.1",
  registration: "disabled",
  max_file_size: 100_000_000,
  supported_text_extensions: ["md"],
  push_debounce_ms: 200,
  capabilities: { history: false, diff: false },
  push_limits: { max_json_bytes: 100_000, max_changes: 1000 }
};
const configB: ServerConfigResponse = {
  ...configA,
  server_name: "B",
  supported_text_extensions: ["md", "canvas"],
  push_debounce_ms: 300,
  capabilities: { history: true, diff: true },
  push_limits: { max_json_bytes: 2_097_152, max_changes: 1000 }
};

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

function createConfigHarness() {
  const responses: Promise<ServerConfigResponse>[] = [];
  const plugin = Object.create(PKVSyncPlugin.prototype) as ConfigPluginHarness;
  plugin.settings = { ...DEFAULT_SETTINGS, ...identityA };
  plugin.manifest = { version: "1.7.1" };
  plugin.serverCapabilities = null;
  plugin.serverPushLimits = { ...LEGACY_PUSH_LIMITS };
  plugin.persistenceRecoveryRequired = false;
  plugin.updateStatus = vi.fn();
  plugin.invalidateSyncEngine = vi.fn();
  const rebuild = vi.fn();
  plugin.rebuildSyncEngine = rebuild;
  (
    plugin as unknown as { authStore: { save(value: unknown): void } }
  ).authStore = { save: () => undefined };

  let disk: unknown = { settings: { ...DEFAULT_SETTINGS }, syncIndexes: {} };
  let holdNextSave: (() => Promise<void>) | null = null;
  plugin.dataStore = new SerializedPluginDataStore(
    async () => structuredClone(disk),
    async (next) => {
      const hold = holdNextSave;
      holdNextSave = null;
      if (hold) await hold();
      disk = structuredClone(next);
    }
  );

  vi.mocked(requestUrl).mockImplementation((() => {
    const queued = responses.shift();
    if (!queued) throw new Error("No config response was queued");
    return queued.then((data) => ({
      status: 200,
      headers: { "content-type": "application/json" },
      json: data,
      text: JSON.stringify(data),
      arrayBuffer: new ArrayBuffer(0)
    }));
  }) as unknown as typeof requestUrl);
  const noticeStart = notices.length;
  return {
    plugin,
    respondNext(response: Promise<ServerConfigResponse>) {
      responses.push(response);
    },
    waitForRequests(count: number) {
      return vi.waitFor(() => expect(requestUrl).toHaveBeenCalledTimes(count));
    },
    changeConnection(identity: ConnectionIdentity) {
      return plugin.setConnectionPatch(identity);
    },
    async respondAndRefresh(config: ServerConfigResponse) {
      responses.push(Promise.resolve(config));
      return plugin.refreshServerCapabilities();
    },
    /**
     * Holds the next PluginData write (and therefore the publish gate) until
     * the returned release() is called.
     */
    holdNextSave() {
      const gate = deferred<void>();
      holdNextSave = () => gate.promise;
      return {
        release() {
          gate.resolve();
        }
      };
    },
    currentPublishedConfig() {
      return {
        push_limits: plugin.currentPushLimits(),
        capabilities: plugin.serverCapabilities,
        supported_text_extensions: plugin.settings.textExtensions,
        push_debounce_ms: plugin.settings.debounceMs
      };
    },
    savedSettings() {
      return readPluginSettings(disk);
    },
    oldScopeNotifications() {
      return notices.slice(noticeStart);
    },
    rebuildCount() {
      return rebuild.mock.calls.length;
    }
  };
}

beforeEach(() => {
  vi.mocked(requestUrl).mockReset();
  notices.length = 0;
});

describe("refreshServerCapabilities mirrors server config (S6)", () => {
  it("adopts the server's text extension set when it differs", async () => {
    const f = createConfigHarness();
    const cfg = await f.respondAndRefresh({
      ...configB,
      push_debounce_ms: DEFAULT_SETTINGS.debounceMs
    });

    expect(cfg).not.toBeNull();
    expect(f.plugin.settings.textExtensions).toEqual(["md", "canvas"]);
    expect(f.savedSettings().textExtensions).toEqual(["md", "canvas"]);
    expect(f.rebuildCount()).toBe(1);
  });

  it("does not rebuild when the server only reorders the extension list (W6)", async () => {
    const f = createConfigHarness();
    const reordered = [...DEFAULT_SETTINGS.textExtensions].reverse();
    await f.respondAndRefresh({
      ...configB,
      supported_text_extensions: reordered,
      push_debounce_ms: DEFAULT_SETTINGS.debounceMs
    });

    expect(f.plugin.settings.textExtensions).toEqual(
      DEFAULT_SETTINGS.textExtensions
    );
    expect(f.savedSettings().textExtensions).toEqual(
      DEFAULT_SETTINGS.textExtensions
    );
    expect(f.rebuildCount()).toBe(0);
  });

  it("does not rebuild when the caller forbids it (W7 migration path)", async () => {
    const f = createConfigHarness();
    f.respondNext(Promise.resolve(configB));
    const cfg = await f.plugin.refreshServerCapabilities(false);

    expect(cfg).not.toBeNull();
    // The mirror still persists the new set...
    expect(f.savedSettings().textExtensions).toEqual(["md", "canvas"]);
    // ...but must not start an engine before the migrated index is saved.
    expect(f.rebuildCount()).toBe(0);
  });

  it("does not rewrite settings when the set already matches", async () => {
    const f = createConfigHarness();
    await f.respondAndRefresh({
      ...configB,
      supported_text_extensions: [...DEFAULT_SETTINGS.textExtensions],
      push_debounce_ms: DEFAULT_SETTINGS.debounceMs
    });

    expect(f.savedSettings().textExtensions).toEqual(
      DEFAULT_SETTINGS.textExtensions
    );
    expect(f.rebuildCount()).toBe(0);
  });
});

describe("refreshServerCapabilities scope protection", () => {
  it("does not let server A overwrite server B after its delayed response", async () => {
    const a = deferred<ServerConfigResponse>();
    const f = createConfigHarness();
    f.respondNext(a.promise);
    const old = f.plugin.refreshServerCapabilities();
    await f.waitForRequests(1);
    await f.changeConnection(identityB);
    await f.respondAndRefresh(configB);
    const rebuiltBeforeOldResponse = f.rebuildCount();
    a.resolve(configA);
    await old;
    expect(f.currentPublishedConfig()).toEqual({
      push_limits: configB.push_limits,
      capabilities: configB.capabilities,
      supported_text_extensions: configB.supported_text_extensions,
      push_debounce_ms: configB.push_debounce_ms
    });
    expect(f.savedSettings()).toMatchObject({
      textExtensions: configB.supported_text_extensions
    });
    expect(f.oldScopeNotifications()).toEqual([]);
    expect(f.rebuildCount()).toBe(rebuiltBeforeOldResponse);
  });

  it("does not let a delayed failure clear server B's config", async () => {
    const a = deferred<ServerConfigResponse>();
    const f = createConfigHarness();
    f.respondNext(a.promise);
    const old = f.plugin.refreshServerCapabilities();
    await f.waitForRequests(1);
    await f.changeConnection(identityB);
    await f.respondAndRefresh(configB);
    const rebuiltBeforeOldResponse = f.rebuildCount();

    a.reject(new Error("network down"));
    await expect(old).resolves.toBeNull();

    expect(f.currentPublishedConfig()).toMatchObject({
      push_limits: configB.push_limits,
      capabilities: configB.capabilities
    });
    expect(f.oldScopeNotifications()).toEqual([]);
    expect(f.rebuildCount()).toBe(rebuiltBeforeOldResponse);
  });

  it("drops a response that waited behind the publish queue of a newer connection", async () => {
    const f = createConfigHarness();
    const a = deferred<ServerConfigResponse>();
    f.respondNext(a.promise);
    const old = f.plugin.refreshServerCapabilities();
    await f.waitForRequests(1);

    const hold = f.holdNextSave();
    const switching = f.plugin.setSelectedVault("held-vault", "Held");
    const changing = f.changeConnection(identityB);
    a.resolve(configA);
    await Promise.resolve();
    hold.release();
    await Promise.all([switching, changing, old]);

    expect(f.plugin.settings.textExtensions).toEqual(DEFAULT_SETTINGS.textExtensions);
    expect(f.savedSettings().textExtensions).toEqual(
      DEFAULT_SETTINGS.textExtensions
    );
    expect(f.plugin.serverCapabilities).toBeNull();
    expect(f.oldScopeNotifications()).toEqual([]);
  });

  it("drops a response captured before A to B to A", async () => {
    const a = deferred<ServerConfigResponse>();
    const f = createConfigHarness();
    f.respondNext(a.promise);
    const old = f.plugin.refreshServerCapabilities();
    await f.waitForRequests(1);
    await f.changeConnection(identityB);
    await f.changeConnection(identityA);
    a.resolve(configA);
    await expect(old).resolves.toBeNull();

    expect(f.plugin.serverCapabilities).toBeNull();
    expect(f.plugin.currentPushLimits()).toEqual(LEGACY_PUSH_LIMITS);
    expect(f.savedSettings().textExtensions).toEqual(
      DEFAULT_SETTINGS.textExtensions
    );
  });

  it("drops a pending refresh when only the token rotates", async () => {
    const a = deferred<ServerConfigResponse>();
    const f = createConfigHarness();
    f.respondNext(a.promise);
    const old = f.plugin.refreshServerCapabilities();
    await f.waitForRequests(1);
    await f.changeConnection({ ...identityA, token: "rotated" });
    a.resolve(configA);
    await expect(old).resolves.toBeNull();

    expect(f.plugin.serverCapabilities).toBeNull();
    expect(f.savedSettings().textExtensions).toEqual(
      DEFAULT_SETTINGS.textExtensions
    );
  });

  it("drops a pending refresh when only the deployment key changes", async () => {
    const a = deferred<ServerConfigResponse>();
    const f = createConfigHarness();
    f.respondNext(a.promise);
    const old = f.plugin.refreshServerCapabilities();
    await f.waitForRequests(1);
    await f.changeConnection({ ...identityA, deploymentKey: "key-rotated" });
    a.resolve(configA);
    await expect(old).resolves.toBeNull();

    expect(f.plugin.serverCapabilities).toBeNull();
    expect(f.currentPublishedConfig().push_limits).toEqual(LEGACY_PUSH_LIMITS);
  });

  it("keeps the last valid config when a same-scope refresh fails", async () => {
    const f = createConfigHarness();
    await f.respondAndRefresh(configB);
    const before = f.currentPublishedConfig();

    const failed = deferred<ServerConfigResponse>();
    f.respondNext(failed.promise);
    const refreshing = f.plugin.refreshServerCapabilities();
    await f.waitForRequests(2);
    failed.reject(new Error("timeout"));
    await expect(refreshing).resolves.toBeNull();

    expect(f.currentPublishedConfig()).toEqual(before);
  });

  it("applies a newer same-scope response that arrives after an older one", async () => {
    const first = deferred<ServerConfigResponse>();
    const f = createConfigHarness();
    f.respondNext(first.promise);
    const older = f.plugin.refreshServerCapabilities();
    await f.waitForRequests(1);
    const newer = f.respondAndRefresh(configB);
    first.resolve(configA);
    await older;
    await newer;

    expect(f.currentPublishedConfig()).toMatchObject({
      push_limits: configB.push_limits,
      capabilities: configB.capabilities
    });
  });
});
