import { afterEach, describe, expect, it, vi } from "vitest";
import type { LocalIndex } from "../../src/sync/types";

/**
 * W4: the poll/debounce paths fire syncNow() without an awaiter. The
 * status element already reports a failed pass, but the rejection must
 * still be consumed — otherwise every timer tick while offline produced
 * an unhandled promise rejection.
 */
describe("SyncOrchestrator timer kicks (W4)", () => {
  afterEach(async () => {
    // rebuild() starts a REAL subscribeVaultEvents loop whose async error
    // path (fetch -> onError -> debugLog) dereferences the stubbed window
    // after the assertions finish. Give those microtasks a tick while the
    // stub still exists, then tear down.
    await new Promise((resolve) => setTimeout(resolve, 20));
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("consumes the rejection of a poll-triggered sync", async () => {
    const intervals: Array<() => void> = [];
    vi.stubGlobal("window", {
      setInterval: (cb: () => void) => {
        intervals.push(cb);
        return intervals.length;
      },
      clearInterval: vi.fn(),
      setTimeout: globalThis.setTimeout,
      clearTimeout: globalThis.clearTimeout,
      // debugLog dereferences window.process.env.NODE_ENV on the SSE
      // error path; without this the stub itself throws inside an async
      // callback and vitest reports an unhandled rejection (which fails
      // the coverage CI job).
      process: { env: { NODE_ENV: "test" } }
    });

    const syncClientModule = await import("../../src/api/sync-client");
    const vaultAdapterModule = await import("../../src/sync/vault-adapter");
    const syncApiSpy = vi
      .spyOn(syncClientModule, "SyncApi")
      .mockImplementation(function MockSyncApi(this: Record<string, unknown>) {
        this.pull = async () => ({
          from: null,
          to: null,
          added: [],
          modified: [],
          deleted: []
        });
        this.state = vi.fn();
        this.uploadCheck = vi.fn().mockResolvedValue({ missing: [] });
        this.uploadBlob = vi.fn();
        this.push = vi.fn();
        this.downloadBlob = vi.fn();
        this.downloadTextFile = vi.fn();
        this.getVaultSettings = vi.fn();
      } as unknown as typeof syncClientModule.SyncApi);
    const adapterSpy = vi
      .spyOn(vaultAdapterModule, "ObsidianVaultAdapter")
      .mockImplementation(function MockAdapter(this: Record<string, unknown>) {
        this.scan = async () => [];
        this.exists = () => false;
        this.readText = async () => "";
        this.writeText = async () => undefined;
        this.writeBinary = async () => undefined;
        this.trash = async () => undefined;
        this.listFiles = () => [];
        this.snapshot = async () => {
          throw new Error("not needed");
        };
      } as unknown as typeof vaultAdapterModule.ObsidianVaultAdapter);

    const plugin = {
      syncGeneration: 0,
      pushDebouncer: null,
      pollTimer: null,
      fallbackTimer: null,
      engine: null,
      api: vi.fn(() => ({})),
      loadSyncIndex: async (): Promise<LocalIndex> => ({
        lastSyncedCommit: null,
        files: {}
      }),
      updateSyncIndex: async () => undefined,
      statusEl: null,
      text: () => ({
        statusConnected: "connected",
        statusSyncing: "syncing",
        statusOffline: "offline",
        statusError: "error",
        statusNotConfigured: "not configured"
      }),
      recordSyncSuccess: vi.fn(),
      settings: {
        serverUrl: "https://sync.example.com",
        deploymentKey: "k",
        token: "t",
        userId: "u",
        selectedVaultId: "v1",
        deviceName: "d",
        textExtensions: ["md"],
        extraExcludeGlobs: [],
        debounceMs: 250,
        pollIntervalSeconds: 30,
        deviceId: "dev"
      },
      manifest: { version: "1.7.0" },
      app: { vault: {} }
    };

    const { SyncOrchestrator } = await import("../../src/sync-orchestrator");
    const orchestrator = new SyncOrchestrator(plugin as never);
    const unhandled: unknown[] = [];
    const onUnhandled = (reason: unknown): void => {
      unhandled.push(reason);
    };
    process.on("unhandledRejection", onUnhandled);
    try {
      orchestrator.rebuild();
      // Replace the engine with one whose pass always fails, the way an
      // offline poll tick behaves.
      plugin.engine = {
        syncNow: () => Promise.reject(new Error("offline")),
        stopEventSubscription: vi.fn(),
        abort: vi.fn()
      } as never;
      // rebuild() registered two intervals: poll first, fallback second.
      expect(intervals.length).toBeGreaterThanOrEqual(2);
      intervals[0]();
      await new Promise((resolve) => setTimeout(resolve, 50));
      expect(unhandled).toEqual([]);
    } finally {
      process.off("unhandledRejection", onUnhandled);
      orchestrator.invalidate();
      syncApiSpy.mockRestore();
      adapterSpy.mockRestore();
    }
  });

  it("refreshes the server config every tenth poll tick (W6)", async () => {
    const intervals: Array<() => void> = [];
    vi.stubGlobal("window", {
      setInterval: (cb: () => void) => {
        intervals.push(cb);
        return intervals.length;
      },
      clearInterval: vi.fn(),
      setTimeout: globalThis.setTimeout,
      clearTimeout: globalThis.clearTimeout,
      // debugLog dereferences window.process.env.NODE_ENV on the SSE
      // error path; without this the stub itself throws inside an async
      // callback and vitest reports an unhandled rejection (which fails
      // the coverage CI job).
      process: { env: { NODE_ENV: "test" } }
    });

    const syncClientModule = await import("../../src/api/sync-client");
    const vaultAdapterModule = await import("../../src/sync/vault-adapter");
    const syncApiSpy = vi
      .spyOn(syncClientModule, "SyncApi")
      .mockImplementation(function MockSyncApi(this: Record<string, unknown>) {
        this.pull = async () => ({
          from: null,
          to: null,
          added: [],
          modified: [],
          deleted: []
        });
        this.state = vi.fn();
        this.uploadCheck = vi.fn().mockResolvedValue({ missing: [] });
        this.uploadBlob = vi.fn();
        this.push = vi.fn();
        this.downloadBlob = vi.fn();
        this.downloadTextFile = vi.fn();
        this.getVaultSettings = vi.fn();
      } as unknown as typeof syncClientModule.SyncApi);
    const adapterSpy = vi
      .spyOn(vaultAdapterModule, "ObsidianVaultAdapter")
      .mockImplementation(function MockAdapter(this: Record<string, unknown>) {
        this.scan = async () => [];
        this.exists = () => false;
        this.readText = async () => "";
        this.writeText = async () => undefined;
        this.writeBinary = async () => undefined;
        this.trash = async () => undefined;
        this.listFiles = () => [];
        this.snapshot = async () => {
          throw new Error("not needed");
        };
      } as unknown as typeof vaultAdapterModule.ObsidianVaultAdapter);

    const refreshServerCapabilities = vi.fn().mockResolvedValue(undefined);
    const plugin = {
      syncGeneration: 0,
      pushDebouncer: null,
      pollTimer: null,
      fallbackTimer: null,
      engine: null,
      api: vi.fn(() => ({})),
      loadSyncIndex: async (): Promise<LocalIndex> => ({
        lastSyncedCommit: null,
        files: {}
      }),
      updateSyncIndex: async () => undefined,
      statusEl: null,
      text: () => ({
        statusConnected: "connected",
        statusSyncing: "syncing",
        statusOffline: "offline",
        statusError: "error",
        statusNotConfigured: "not configured"
      }),
      refreshServerCapabilities,
      recordSyncSuccess: vi.fn(),
      settings: {
        serverUrl: "https://sync.example.com",
        deploymentKey: "k",
        token: "t",
        userId: "u",
        selectedVaultId: "v1",
        deviceName: "d",
        textExtensions: ["md"],
        extraExcludeGlobs: [],
        debounceMs: 250,
        pollIntervalSeconds: 30,
        deviceId: "dev"
      },
      manifest: { version: "1.7.0" },
      app: { vault: {} }
    };

    const { SyncOrchestrator } = await import("../../src/sync-orchestrator");
    const orchestrator = new SyncOrchestrator(plugin as never);
    try {
      orchestrator.rebuild();
      plugin.engine = {
        syncNow: async () => undefined,
        stopEventSubscription: vi.fn(),
        abort: vi.fn()
      } as never;
      expect(intervals.length).toBeGreaterThanOrEqual(2);
      const poll = intervals[0];
      for (let i = 0; i < 9; i += 1) poll();
      expect(refreshServerCapabilities).not.toHaveBeenCalled();
      poll();
      expect(refreshServerCapabilities).toHaveBeenCalledTimes(1);
    } finally {
      orchestrator.invalidate();
      syncApiSpy.mockRestore();
      adapterSpy.mockRestore();
    }
  });
});
