import { describe, expect, it, vi } from "vitest";
import { en } from "../../src/i18n/en";
import { PKVSyncSettingTab } from "../../src/ui/settings-tab";

/**
 * R7R3-1: editing a connection field (server URL / deployment key) is a
 * DRAFT. Every keystroke used to run the full connection commit — epoch
 * bump, persist-to-disk, sync-engine teardown — so deleting one character
 * left the vault with no engine, no timers, a persisted half-typed URL and
 * no recovery path until the next explicit connect.
 *
 * These tests drive the prototype methods with a recording plugin double:
 * the wiring under test is "which plugin entry point does each field event
 * call", which is exactly the regression.
 */

type TabHarness = {
  app: { vault: { configDir: string } };
  plugin: unknown;
  renderConnection(panel: unknown): void;
};

/** Recording double: every plugin entry point appends its name. */
function makePlugin(): {
  plugin: Record<string, unknown>;
  calls: string[];
  settings: Record<string, unknown>;
} {
  const calls: string[] = [];
  const settings: Record<string, unknown> = {
    serverUrl: "https://sync.example.com",
    deploymentKey: "k_fullkey",
    deviceName: "Laptop",
    timezone: "Asia/Shanghai",
    language: "auto",
    themeMode: "dark",
    checkForUpdates: false,
    updateSource: "server",
    lastUpdateCheckAt: 0,
    selectedVaultId: "",
    selectedVaultName: ""
  };
  const plugin: Record<string, unknown> = {
    settings,
    manifest: { version: "1.7.2" },
    text: () => en,
    stageConnectionDraft: (patch: Record<string, string>) => {
      calls.push("draft");
      Object.assign(settings, patch);
    },
    setConnectionPatch: vi.fn(async () => {
      calls.push("commit");
      return null;
    }),
    saveSettings: vi.fn(async () => {
      calls.push("persist");
    }),
    saveSettingsPatch: vi.fn(async () => {
      calls.push("patch-persist");
    }),
    invalidateSyncEngine: vi.fn(() => calls.push("invalidate")),
    rebuildSyncEngine: vi.fn(),
    api: vi.fn(() => ({ config: vi.fn() })),
    syncNowManual: vi.fn(),
    checkForPluginUpdates: vi.fn(),
    scheduleUpdateChecks: vi.fn(),
    availableUpdate: null,
    serverCapabilities: null,
    cfg: null
  };
  return { plugin, calls, settings };
}

function makeTab(plugin: Record<string, unknown>): TabHarness {
  const tab = Object.create(
    PKVSyncSettingTab.prototype
  ) as unknown as TabHarness;
  tab.app = { vault: { configDir: ".obsidian" } };
  tab.plugin = plugin as never;
  return tab;
}

/**
 * Intercept renderTextField at the prototype boundary — the exact seam
 * where field events are wired — and capture the serverUrl field's
 * options. Everything else in renderConnection renders against a
 * permissive dummy panel.
 */
function captureFirstTextField(tab: TabHarness): {
  onInput?: (value: string) => void;
  onCommit?: () => Promise<void>;
} {
  const proto = Object.getPrototypeOf(tab) as Record<string, unknown>;
  const original = proto.renderTextField as (
    parent: unknown,
    label: string,
    options: Record<string, unknown>
  ) => unknown;
  let captured: Record<string, unknown> | null = null;
  proto.renderTextField = function patched(
    this: unknown,
    parent: unknown,
    label: string,
    options: Record<string, unknown>
  ) {
    if (!captured && label === en.serverUrl) {
      captured = options;
      return { value: options.value ?? "" };
    }
    return original.call(this, parent, label, options);
  } as typeof original;
  const panel = new Proxy(
    {},
    {
      get: (_t, prop) => {
        if (prop === "createDiv" || prop === "createEl" || prop === "createSpan") {
          return () => panel;
        }
        return () => undefined;
      }
    }
  );
  try {
    tab.renderConnection(panel);
  } finally {
    proto.renderTextField = original;
  }
  if (!captured) throw new Error("serverUrl field was not rendered");
  return captured as {
    onInput?: (v: string) => void;
    onCommit?: () => Promise<void>;
  };
}

describe("connection field edits are drafts (R7R3-1)", () => {
  it("a keystroke stages a draft only: no commit, no persist, no teardown", () => {
    const { plugin, calls, settings } = makePlugin();
    const tab = makeTab(plugin);
    const field = captureFirstTextField(tab);

    // The user deletes one character from the URL and stops there.
    field.onInput?.("https://sync.exampl");

    expect(calls).toEqual(["draft"]);
    expect(settings.serverUrl).toBe("https://sync.exampl");
  });

  it("the change/blur event persists without a connection commit or teardown", async () => {
    const { plugin, calls } = makePlugin();
    const tab = makeTab(plugin);
    const field = captureFirstTextField(tab);

    field.onInput?.("https://changed.example.com");
    await field.onCommit?.();

    // persist only — NOT a connection commit and NOT an engine teardown:
    // the engine keeps running on the identity it was built with until an
    // explicit connect switches it.
    expect(calls).toEqual(["draft", "persist"]);
  });

  it("the old per-keystroke commit path is gone from the field wiring", () => {
    // Exhaust every input event: none may reach the commit entry point.
    const { plugin, calls } = makePlugin();
    const tab = makeTab(plugin);
    const field = captureFirstTextField(tab);

    field.onInput?.("https://a.example.com");
    field.onInput?.("https://b.example.com");
    field.onInput?.("https://c.example.com");

    expect(calls).toEqual(["draft", "draft", "draft"]);
    expect(plugin.setConnectionPatch).not.toHaveBeenCalled();
  });
});
