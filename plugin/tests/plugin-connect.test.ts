import { requestUrl } from "obsidian";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import PKVSyncPlugin from "../src/main";
import type { PluginLanguage } from "../src/settings";
import { PKVSyncSettingTab } from "../src/ui/settings-tab";
import { createMigrationHarness } from "./helpers/migration-fixture";
import { notices } from "./mocks/obsidian";

function response(data: unknown, status = 200) {
  return {
    status, headers: { "content-type": "application/json" }, json: data,
    text: JSON.stringify(data), arrayBuffer: new ArrayBuffer(0)
  };
}

function deferredResponse() {
  let release!: (value: ReturnType<typeof response>) => void;
  const promise = new Promise<ReturnType<typeof response>>((resolve) => { release = resolve; });
  return {
    release,
    request: Object.assign(promise, {
      arrayBuffer: promise.then((value) => value.arrayBuffer),
      json: promise.then((value) => value.json),
      text: promise.then((value) => value.text)
    })
  };
}

function connectHarness(language: PluginLanguage = "auto") {
  const h = createMigrationHarness({ settings: { token: "", language } });
  h.plugin.apiForConnection = PKVSyncPlugin.prototype.apiForConnection;
  const tab = Object.create(PKVSyncSettingTab.prototype) as PKVSyncSettingTab;
  let connect!: () => Promise<void>;
  Object.assign(tab, {
    plugin: h.plugin, cfg: null, refresh: vi.fn(),
    renderHeader: vi.fn(), renderLanguage: vi.fn(), renderThemeMode: vi.fn(),
    renderSectionLabel: vi.fn(), renderSelectField: vi.fn(), renderUpdates: vi.fn(),
    renderConflictCleanup: vi.fn(),
    renderTextField: (_panel: unknown, _label: string, field: { value: string }) => ({ value: field.value }),
    renderButton: (_panel: unknown, label: string, _style: string, action: () => Promise<void>) => {
      if (label === h.plugin.text().connect) connect = action;
    }
  });
  (tab as unknown as { renderConnection(panel: unknown): void }).renderConnection({});
  return { ...h, connect };
}

function config(h: ReturnType<typeof connectHarness>) {
  return {
    server_name: "Test", version: "1.7.1", registration: "disabled",
    max_file_size: 100_000_000, supported_text_extensions: h.plugin.settings.textExtensions,
    push_debounce_ms: h.plugin.settings.debounceMs
  };
}

beforeEach(() => { vi.mocked(requestUrl).mockReset(); notices.length = 0; });
afterEach(() => vi.restoreAllMocks());

it.each([
  { code: "setup_required", message: "Initial setup required" },
  { code: "invalid_deployment_key", message: "Deployment key is invalid" }
])("reports the current connection's $code response through the real config method", async ({ code, message }) => {
  const h = connectHarness();
  vi.mocked(requestUrl).mockResolvedValue(response({ error: { code, message } }, 403));

  await h.connect();

  expect(requestUrl).toHaveBeenCalledTimes(1);
  expect(notices).toEqual([code === "setup_required" ? h.plugin.text().setupRequiredNotice : message]);
  expect(h.events).not.toContain("rebuild");
});

it("reports a network failure from the current explicit connection", async () => {
  const h = connectHarness();
  vi.mocked(requestUrl).mockRejectedValue(new Error("Connection timed out"));

  await h.connect();

  expect(notices).toEqual(["Connection timed out"]);
  expect(h.events).not.toContain("rebuild");
});

it("drops a failed explicit connection after switching servers", async () => {
  const h = connectHarness();
  const gate = deferredResponse();
  vi.mocked(requestUrl).mockReturnValueOnce(gate.request);
  const pending = h.connect();
  await vi.waitFor(() => expect(requestUrl).toHaveBeenCalledTimes(1));
  await h.plugin.setConnectionPatch({ serverUrl: "https://new.example.test" });
  gate.release(response({ error: { code: "setup_required", message: "Old setup required" } }, 403));
  await pending;

  expect(notices).toEqual([]);
  expect(h.events).not.toContain("rebuild");
});

it("drops an explicit connection failure superseded by a newer same-scope refresh", async () => {
  const h = connectHarness();
  const gate = deferredResponse();
  vi.mocked(requestUrl).mockReturnValueOnce(gate.request);
  const pending = h.connect();
  await vi.waitFor(() => expect(requestUrl).toHaveBeenCalledTimes(1));
  vi.mocked(requestUrl).mockResolvedValueOnce(response(config(h)));
  await h.plugin.refreshServerCapabilities(false);
  gate.release(response({ error: { code: "setup_required", message: "Old setup required" } }, 403));
  await pending;

  expect(notices).toEqual([]);
  expect(h.events).not.toContain("rebuild");
});

it("resumes the accepted explicit connection exactly once", async () => {
  const h = connectHarness();
  vi.mocked(requestUrl).mockResolvedValue(response(config(h)));

  await h.connect();

  expect(h.events).toEqual(["rebuild"]);
  expect(notices).toEqual([
    h.plugin.text().connectedToServer.replace("{serverName}", "old.example.test")
  ]);
});

describe.each([
  {
    language: "en" as const,
    restored: "Could not save local settings. The previous state was restored. Reason: Local disk full",
    uncertain: "Local state could not be restored. Sync is paused. Check storage and reload the plugin. Reason: Local disk full",
    committed: "Local settings were saved, but applying them failed. Sync is paused. Reload the plugin. Reason: Local publish failed"
  },
  {
    language: "zh-CN" as const,
    restored: "本地设置保存失败，已恢复原状态。原因：Local disk full",
    uncertain: "本地状态恢复失败，同步已暂停。请检查存储并重新加载插件。原因：Local disk full",
    committed: "本地设置已保存，但应用失败，同步已暂停。请重新加载插件。原因：Local publish failed"
  }
])("$language local storage errors", ({ language, restored, uncertain, committed }) => {
  it("reports a rejected save with its cause after verifying the previous state", async () => {
    const h = connectHarness(language);
    h.setSave(async (next) => {
      if (h.saveCalls() === 1) h.setDisk(next);
      else throw new Error("Local disk full");
    });
    vi.mocked(requestUrl).mockResolvedValue(response({
      ...config(h), supported_text_extensions: ["md", "canvas"]
    }));

    await h.connect();

    expect(h.saveCalls()).toBe(2);
    expect(h.savedSettings().textExtensions).toEqual(["md"]);
    expect(h.plugin.persistenceRecoveryRequired).toBe(false);
    expect(h.events).toEqual([]);
    expect(notices).toEqual([restored]);
  });

  it("reports one recovery warning and keeps sync paused when disk state is uncertain", async () => {
    const h = connectHarness(language);
    h.setSave(async (next) => {
      if (h.saveCalls() === 1) h.setDisk(next);
      else throw new Error("Local disk full");
    });
    h.setLoad(async () => {
      if (h.saveCalls() >= 2) throw new Error("Read back failed");
      return h.disk();
    });
    vi.mocked(requestUrl).mockResolvedValue(response({
      ...config(h), supported_text_extensions: ["md", "canvas"]
    }));

    await h.connect();
    await h.plugin.resumeSyncForCurrentScope();

    expect(h.saveCalls()).toBe(3);
    expect(h.plugin.persistenceRecoveryRequired).toBe(true);
    expect(h.events).toEqual(["invalidate"]);
    expect(notices).toEqual([uncertain]);
  });

  it("reports an apply failure without rolling back the saved settings or resuming sync", async () => {
    const h = connectHarness(language);
    Object.defineProperty(h.plugin.settings, "textExtensions", {
      configurable: true, enumerable: true,
      get: () => ["md"],
      set: () => { throw new Error("Local publish failed"); }
    });
    vi.mocked(requestUrl).mockResolvedValue(response({
      ...config(h), supported_text_extensions: ["md", "canvas"]
    }));

    await h.connect();
    await h.plugin.resumeSyncForCurrentScope();

    expect(h.saveCalls()).toBe(2);
    expect(h.savedSettings().textExtensions).toEqual(["md", "canvas"]);
    expect(h.plugin.persistenceRecoveryRequired).toBe(true);
    expect(h.events).toEqual(["invalidate"]);
    expect(notices).toEqual([committed]);
  });
});

it("keeps a verified rollback quiet during background refresh", async () => {
  const h = connectHarness("en");
  h.setSave(async () => { throw new Error("Local disk full"); });
  vi.mocked(requestUrl).mockResolvedValue(response({
    ...config(h), supported_text_extensions: ["md", "canvas"]
  }));

  await expect(h.plugin.refreshServerCapabilities(false)).resolves.toBeNull();

  expect(h.plugin.persistenceRecoveryRequired).toBe(false);
  expect(h.savedSettings().textExtensions).toEqual(["md"]);
  expect(notices).toEqual([]);
});

it("keeps the global storage warning when a newer connection supersedes the failed write", async () => {
  const h = connectHarness("en");
  let release!: () => void;
  h.setSave(async (next) => {
    if (h.saveCalls() === 1) h.setDisk(next);
    else throw new Error("Local disk full");
  });
  h.setLoad(async () => {
    if (h.saveCalls() >= 2) throw new Error("Read back failed");
    return h.disk();
  });
  vi.mocked(requestUrl).mockImplementationOnce((() => {
    release = h.holdNextSave().release;
    return Promise.resolve(response({
      ...config(h), supported_text_extensions: ["md", "canvas"]
    }));
  }) as unknown as typeof requestUrl);
  const pending = h.connect();
  await vi.waitFor(() => expect(h.saveCalls()).toBe(2));
  const switching = h.plugin.setConnectionPatch({ serverUrl: "https://new.example.test" });
  const failedSwitch = switching.catch((error: unknown) => error);
  release();
  await Promise.all([pending, failedSwitch]);
  await h.plugin.resumeSyncForCurrentScope();

  expect(h.plugin.settings.serverUrl).toBe("https://new.example.test");
  expect(h.plugin.persistenceRecoveryRequired).toBe(true);
  expect(h.events).not.toContain("rebuild");
  expect(notices).toEqual([
    "Local state could not be restored. Sync is paused. Check storage and reload the plugin. Reason: Local disk full"
  ]);
});
