import { afterEach, describe, expect, it, vi } from "vitest";
import PKVSyncPlugin from "../src/main";

type EngineHarness = {
  stopEventSubscription(): void;
  flushOnUnload(timeoutMs: number): Promise<void>;
};

type PluginUnloadHarness = {
  pushDebouncer: { cancel(): void } | null;
  engine: EngineHarness | null;
  pollTimer: number | null;
  fallbackTimer: number | null;
  syncGeneration: number;
  statusEl: unknown | null;
  onunload(): void | Promise<void>;
};

describe("PKVSyncPlugin unload", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("flushes pending sync before invalidating the engine", async () => {
    const clearInterval = vi.fn();
    vi.stubGlobal("window", { clearInterval });
    const events: string[] = [];
    const plugin = Object.create(PKVSyncPlugin.prototype) as PluginUnloadHarness;
    plugin.syncGeneration = 7;
    plugin.pollTimer = 11;
    plugin.fallbackTimer = 12;
    plugin.statusEl = {};
    plugin.pushDebouncer = { cancel: () => events.push("cancel") };
    const flushOnUnload = vi.fn(async (timeoutMs: number) => {
      events.push(`flush:${timeoutMs}:${plugin.syncGeneration}`);
    });
    plugin.engine = {
      stopEventSubscription: () => events.push("stop"),
      flushOnUnload
    };

    await plugin.onunload();

    expect(flushOnUnload).toHaveBeenCalledWith(3000);
    expect(events).toEqual(["cancel", "stop", "flush:3000:7"]);
    expect(clearInterval).toHaveBeenCalledWith(11);
    expect(clearInterval).toHaveBeenCalledWith(12);
    expect(plugin.syncGeneration).toBe(8);
    expect(plugin.engine).toBeNull();
    expect(plugin.statusEl).toBeNull();
  });
});
