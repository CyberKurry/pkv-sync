import { afterEach, describe, expect, it, vi } from "vitest";
import { notices } from "../../tests/mocks/obsidian";
import { LEGACY_PUSH_LIMITS } from "../../src/sync/push-limits";
import type { LocalIndex } from "../../src/sync/types";

/**
 * W4: the poll/debounce paths fire syncNow() without an awaiter. The
 * status element already reports a failed pass, but the rejection must
 * still be consumed — otherwise every timer tick while offline produced
 * an unhandled promise rejection.
 */
describe("SyncOrchestrator timer kicks (W4)", () => {
  afterEach(async () => {
    // The tests spy on the real engine, so invalidate() (each test's
    // finally) aborts the real subscribeVaultEvents loop: its in-flight
    // fetch rejects with AbortError, which the loop's catch consumes. One
    // drain tick lets that land while the window stub still exists.
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
      currentPushLimits: () => LEGACY_PUSH_LIMITS,
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
      // Spy on the REAL engine rather than replacing it: a replaced engine
      // leaks the real subscribeVaultEvents loop (invalidate() would call
      // the fake's no-op stopEventSubscription), and its in-flight fetch
      // then errors into onError after the window stub is gone — vitest
      // reports an unhandled rejection and the coverage job fails on it.
      const realEngine = plugin.engine as unknown as {
        syncNow: () => Promise<void>;
      };
      vi.spyOn(realEngine, "syncNow").mockRejectedValue(new Error("offline"));
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
      currentPushLimits: () => LEGACY_PUSH_LIMITS,
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
      // Spy on the real engine (not a replacement) so invalidate() tears
      // down the real subscription — see the W4 test above.
      const realEngine = plugin.engine as unknown as {
        syncNow: () => Promise<void>;
      };
      vi.spyOn(realEngine, "syncNow").mockResolvedValue(undefined);
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

  it("notices once when consecutive timer passes fail on the same error (R9-7)", async () => {
    const intervals: Array<() => void> = [];
    vi.stubGlobal("window", {
      setInterval: (cb: () => void) => {
        intervals.push(cb);
        return intervals.length;
      },
      clearInterval: vi.fn(),
      setTimeout: globalThis.setTimeout,
      clearTimeout: globalThis.clearTimeout,
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
      currentPushLimits: () => LEGACY_PUSH_LIMITS,
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
        statusNotConfigured: "not configured",
        syncRepeatedlyFailing:
          "Sync keeps failing on the same error. Reason: {reason}"
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
    notices.length = 0;
    const onUnhandled = (): void => undefined;
    process.on("unhandledRejection", onUnhandled);
    try {
      orchestrator.rebuild();
      const realEngine = plugin.engine as unknown as {
        syncNow: () => Promise<void>;
      };
      // Let rebuild()'s own in-flight kickSync pass settle BEFORE spying:
      // with R11-3 a success clears the failure streak, so the initial
      // (successful, empty-vault) pass finishing mid-test would reset the
      // streak the poll passes below are building up.
      await new Promise((resolve) => setTimeout(resolve, 50));
      const stuck = new Error("File not found: blocked.md");
      const syncSpy = vi
        .spyOn(realEngine, "syncNow")
        .mockRejectedValue(stuck);
      const poll = intervals[0];
      // Pass 1: consumed silently (status bar shows it).
      poll();
      await new Promise((resolve) => setTimeout(resolve, 20));
      expect(notices).toEqual([]);
      // Pass 2: same error again → exactly one notice.
      poll();
      await new Promise((resolve) => setTimeout(resolve, 20));
      expect(notices).toEqual([
        "Sync keeps failing on the same error. Reason: File not found: blocked.md"
      ]);
      // Pass 3+: still the same error → no repetition.
      poll();
      await new Promise((resolve) => setTimeout(resolve, 20));
      expect(notices.length).toBe(1);
      // A DIFFERENT error re-arms the dedup.
      syncSpy.mockRejectedValue(new Error("HTTP 500"));
      poll();
      await new Promise((resolve) => setTimeout(resolve, 20));
      poll();
      await new Promise((resolve) => setTimeout(resolve, 20));
      expect(notices.length).toBe(2);
      expect(notices[1]).toContain("HTTP 500");
    } finally {
      process.off("unhandledRejection", onUnhandled);
      orchestrator.invalidate();
      syncApiSpy.mockRestore();
      adapterSpy.mockRestore();
    }
  });

  it("resets the failure streak on success (R11-3)", async () => {
    // fail → succeed → same fail again must NOT re-notify: "keeps
    // failing" means consecutive failures.
    const intervals: Array<() => void> = [];
    vi.stubGlobal("window", {
      setInterval: (cb: () => void) => {
        intervals.push(cb);
        return intervals.length;
      },
      clearInterval: vi.fn(),
      setTimeout: globalThis.setTimeout,
      clearTimeout: globalThis.clearTimeout,
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
      currentPushLimits: () => LEGACY_PUSH_LIMITS,
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
        statusNotConfigured: "not configured",
        syncRepeatedlyFailing:
          "Sync keeps failing on the same error. Reason: {reason}"
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
    notices.length = 0;
    const onUnhandled = (): void => undefined;
    process.on("unhandledRejection", onUnhandled);
    try {
      orchestrator.rebuild();
      const realEngine = plugin.engine as unknown as {
        syncNow: () => Promise<void>;
      };
      const stuck = new Error("transient blip");
      const syncSpy = vi
        .spyOn(realEngine, "syncNow")
        .mockRejectedValueOnce(stuck)
        .mockResolvedValueOnce(undefined)
        .mockRejectedValueOnce(new Error("transient blip"));
      const poll = intervals[0];

      poll();
      await new Promise((resolve) => setTimeout(resolve, 20));
      poll(); // succeeds
      await new Promise((resolve) => setTimeout(resolve, 20));
      poll(); // same message as the FIRST failure
      await new Promise((resolve) => setTimeout(resolve, 20));

      expect(syncSpy).toHaveBeenCalledTimes(3);
      expect(notices).toEqual([]);
    } finally {
      process.off("unhandledRejection", onUnhandled);
      orchestrator.invalidate();
      syncApiSpy.mockRestore();
      adapterSpy.mockRestore();
    }
  });
});
