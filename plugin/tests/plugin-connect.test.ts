import { requestUrl } from "obsidian";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import PKVSyncPlugin from "../src/main";
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

function connectHarness() {
  const h = createMigrationHarness({ settings: { token: "" } });
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
