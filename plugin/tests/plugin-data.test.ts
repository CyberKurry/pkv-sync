import { describe, expect, it } from "vitest";
import {
  readPluginSettings,
  readSyncIndex,
  syncScopeKey,
  writePluginSettings,
  writeSyncIndex
} from "../src/plugin-data";
import { DEFAULT_SETTINGS } from "../src/settings";
import type { LocalIndex } from "../src/sync/types";

describe("plugin-data", () => {
  it("reads legacy top-level settings", () => {
    expect(readPluginSettings({ serverUrl: "https://sync.example.com" }).serverUrl)
      .toBe("https://sync.example.com");
  });

  it("prefers nested settings", () => {
    expect(
      readPluginSettings({
        serverUrl: "legacy",
        settings: { serverUrl: "nested" }
      }).serverUrl
    ).toBe("nested");
  });

  it("writes settings without dropping sync index", () => {
    const index: LocalIndex = { lastSyncedCommit: "c1", files: {} };

    expect(
      writePluginSettings({ syncIndexes: { a: index } }, DEFAULT_SETTINGS)
    ).toEqual({
      syncIndexes: { a: index },
      settings: DEFAULT_SETTINGS
    });
  });

  it("scopes sync index by server, deployment, account, and vault", () => {
    const a = syncScopeKey({
      ...DEFAULT_SETTINGS,
      serverUrl: "https://one.example.com",
      deploymentKey: "k_one",
      userId: "u1",
      selectedVaultId: "v1"
    });
    const b = syncScopeKey({
      ...DEFAULT_SETTINGS,
      serverUrl: "https://one.example.com",
      deploymentKey: "k_one",
      userId: "u1",
      selectedVaultId: "v2"
    });
    const idxA: LocalIndex = { lastSyncedCommit: "c1", files: {} };
    const idxB: LocalIndex = { lastSyncedCommit: "c2", files: {} };

    const data = writeSyncIndex(
      writeSyncIndex({ settings: DEFAULT_SETTINGS }, a, idxA),
      b,
      idxB
    );

    expect(readSyncIndex(data, a).lastSyncedCommit).toBe("c1");
    expect(readSyncIndex(data, b).lastSyncedCommit).toBe("c2");
  });

  it("does not reuse legacy global sync index for a scoped engine", () => {
    const index: LocalIndex = { lastSyncedCommit: "legacy", files: {} };
    const scoped = syncScopeKey({
      ...DEFAULT_SETTINGS,
      serverUrl: "https://one.example.com",
      deploymentKey: "k_one",
      userId: "u1",
      selectedVaultId: "v1"
    });

    expect(readSyncIndex({ syncIndex: index }, scoped)).toEqual({
      lastSyncedCommit: null,
      files: {}
    });
  });
});
