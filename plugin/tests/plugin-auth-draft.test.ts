import { describe, expect, it, vi } from "vitest";
import type { ConnectionIdentity } from "../src/connection-context";
import { createMigrationHarness } from "./helpers/migration-fixture";

/**
 * R8-1: a half-typed connection-field draft must never reach the auth
 * store — it is the ONLY persistence layer for connection fields
 * (writePluginSettingsWithoutAuth strips them from data.json), and on
 * reload loadSettings() overwrites settings from it. Before the fix, the
 * field's change/blur event ran saveSettings → persistAuth(), so blurring
 * a field containing one typed character permanently replaced k_xxx with
 * "k", with no in-plugin path back.
 */

interface AuthRecord {
  deviceId: string;
  serverUrl: string;
  deploymentKey: string | null;
  token: string | null;
  userId: string | null;
}

function harnessWithAuthStore(initial: AuthRecord) {
  const h = createMigrationHarness({
    settings: {
      token: "token-old",
      serverUrl: initial.serverUrl,
      deploymentKey: initial.deploymentKey ?? ""
    }
  });
  const saved: AuthRecord[] = [];
  let stored: AuthRecord = { ...initial };
  (h.plugin as unknown as { authStore: unknown }).authStore = {
    save: (value: AuthRecord) => {
      saved.push(structuredClone(value));
      stored = value;
    },
    load: () => structuredClone(stored)
  };
  // Real flow: loadSettings() builds the connection context ONCE from the
  // merged settings+auth, before any draft can exist. Reach in and build
  // it the same way so the harness exercises the real invariant.
  h.plugin.captureConnection();
  return { ...h, savedAuth: () => saved, authStore: () => stored };
}

describe("connection drafts never reach the auth store (R8-1)", () => {
  it("a blur-triggered save does not persist a drafted deployment key", async () => {
    const h = harnessWithAuthStore({
      deviceId: "dev-1",
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_fullkey",
      token: "token-old",
      userId: "user-1"
    });
    const plugin = h.plugin;

    // The user selects-all in the deployment key field, types one
    // character (draft = "k"), then clicks elsewhere (change/blur fires
    // the field's onCommit → saveSettings({rebuild:false})).
    plugin.stageConnectionDraft({ deploymentKey: "k" });
    await plugin.saveSettings({ rebuild: false });

    for (const record of h.savedAuth()) {
      expect(record.deploymentKey).toBe("k_fullkey");
      expect(record.serverUrl).toBe("https://sync.example.com");
    }
  });

  it("a blur-triggered save does not persist a drafted server URL", async () => {
    const h = harnessWithAuthStore({
      deviceId: "dev-1",
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_fullkey",
      token: null,
      userId: null
    });
    const plugin = h.plugin;

    plugin.stageConnectionDraft({ serverUrl: "https://half" });
    await plugin.saveSettings({ rebuild: false });

    for (const record of h.savedAuth()) {
      expect(record.serverUrl).toBe("https://sync.example.com");
    }
  });

  it("setConnectionPatch remains the single persistence path for connection fields", async () => {
    const h = harnessWithAuthStore({
      deviceId: "dev-1",
      serverUrl: "https://old.example.com",
      deploymentKey: "k_old",
      token: null,
      userId: null
    });
    const plugin = h.plugin;
    (plugin as unknown as { refreshServerCapabilities: unknown }).refreshServerCapabilities =
      vi.fn(async () => null);

    // The explicit commit (Connect button) still persists the new identity.
    await plugin.setConnectionPatch({
      serverUrl: "https://new.example.com",
      deploymentKey: "k_new"
    });
    await plugin.saveSettings({ rebuild: false });

    const last = h.savedAuth()[h.savedAuth().length - 1];
    expect(last.serverUrl).toBe("https://new.example.com");
    expect(last.deploymentKey).toBe("k_new");
  });

  it("a draft left uncommitted does not become the identity after reload", async () => {
    const h = harnessWithAuthStore({
      deviceId: "dev-1",
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_fullkey",
      token: "token-old",
      userId: "user-1"
    });
    const plugin = h.plugin;

    plugin.stageConnectionDraft({ deploymentKey: "k" });
    await plugin.saveSettings({ rebuild: false });

    // What loadSettings() would restore on next launch: the auth store's
    // connection fields must still be the committed identity.
    const identity: ConnectionIdentity = {
      serverUrl: h.authStore().serverUrl,
      deploymentKey: h.authStore().deploymentKey ?? "",
      token: h.authStore().token ?? "",
      userId: h.authStore().userId ?? "",
      username: ""
    };
    expect(identity.deploymentKey).toBe("k_fullkey");
  });
});
