import { afterEach, describe, expect, it, vi } from "vitest";
import PKVSyncPlugin from "../src/main";

type WatcherHarness = {
  app: {
    vault: {
      on(event: string, callback: (file: unknown) => void): unknown;
    };
    workspace: {
      on(event: string, callback: () => void): unknown;
    };
  };
  pushDebouncer: { trigger(): void } | null;
  engine: { syncNow(): Promise<void> } | null;
  registerEvent(eventRef: unknown): void;
  registerDomEvent(target: unknown, event: string, callback: () => void): void;
  registerVaultWatchers(): void;
};

describe("PKVSyncPlugin vault watchers", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("syncs only for file changes and debounces window blur", () => {
    const vaultCallbacks = new Map<string, (file: unknown) => void>();
    const callbacks: {
      activeLeaf?: () => void;
      blur?: () => void;
    } = {};
    const trigger = vi.fn();
    const syncNow = vi.fn(async () => undefined);
    vi.stubGlobal("window", {});
    const plugin = Object.create(PKVSyncPlugin.prototype) as WatcherHarness;
    plugin.pushDebouncer = { trigger };
    plugin.engine = { syncNow };
    plugin.app = {
      vault: {
        on: vi.fn((event: string, callback: (file: unknown) => void) => {
          vaultCallbacks.set(event, callback);
          return { event };
        })
      },
      workspace: {
        on: vi.fn((event: string, callback: () => void) => {
          if (event === "active-leaf-change") callbacks.activeLeaf = callback;
          return { event };
        })
      }
    };
    plugin.registerEvent = vi.fn();
    plugin.registerDomEvent = vi.fn((_target, event, callback) => {
      if (event === "blur") callbacks.blur = callback;
    });

    plugin.registerVaultWatchers();
    vaultCallbacks.get("modify")?.({ path: "note.md" });
    expect(trigger).toHaveBeenCalledTimes(1);

    vaultCallbacks.get("create")?.({ path: ".git/config" });
    expect(trigger).toHaveBeenCalledTimes(1);

    expect(plugin.app.workspace.on).not.toHaveBeenCalledWith(
      "active-leaf-change",
      expect.anything()
    );
    expect(trigger).toHaveBeenCalledTimes(1);

    callbacks.blur?.();
    expect(trigger).toHaveBeenCalledTimes(2);
    expect(syncNow).not.toHaveBeenCalled();
  });
});
