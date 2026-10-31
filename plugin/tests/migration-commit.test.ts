import { describe, expect, it } from "vitest";
import {
  readPluginSettings,
  readSyncIndex,
  writePluginSettingsPatch,
  writeSyncIndex
} from "../src/plugin-data";
import { DEFAULT_SETTINGS } from "../src/settings";
import { writeMigrationCompletion } from "../src/sync/migration-commit";
import type { LocalIndex } from "../src/sync/types";

const targetIndex: LocalIndex = {
  lastSyncedCommit: "target-head",
  files: {
    "note.md": {
      lastSyncedHash: "hash-note",
      lastSyncedAt: 1_700_000_000,
      kind: "text",
      size: 12
    }
  }
};

const oldIndex: LocalIndex = { lastSyncedCommit: "old-head", files: {} };

const selection = {
  selectedVaultId: "vault-new",
  selectedVaultName: "New Vault",
  lastSyncSuccessAt: 1_700_000_123
};

describe("writeMigrationCompletion", () => {
  it("writes the selection and the target index into one record", () => {
    const raw = writeSyncIndex(
      {
        token: "legacy-token",
        settings: {
          ...DEFAULT_SETTINGS,
          serverUrl: "https://sync.example.test",
          deploymentKey: "secret",
          userId: "u1",
          selectedVaultId: "vault-old",
          debounceMs: 750
        }
      },
      "scope-old",
      oldIndex
    );

    const data = writeMigrationCompletion(raw, selection, "scope-target", targetIndex);

    expect(readPluginSettings(data).selectedVaultId).toBe("vault-new");
    expect(readPluginSettings(data).selectedVaultName).toBe("New Vault");
    expect(readPluginSettings(data).lastSyncSuccessAt).toBe(1_700_000_123);
    expect(readPluginSettings(data).debounceMs).toBe(750);
    expect(readSyncIndex(data, "scope-target")).toEqual(targetIndex);
    expect(readSyncIndex(data, "scope-old")).toEqual(oldIndex);
    expect(JSON.stringify(data)).not.toContain("legacy-token");
    expect(JSON.stringify(data)).not.toContain("secret");
  });

  it("keys the index by the explicit target scope, not the current selection", () => {
    const raw = writeSyncIndex(
      {
        settings: {
          ...DEFAULT_SETTINGS,
          serverUrl: "https://sync.example.test",
          userId: "u1",
          selectedVaultId: "vault-old"
        }
      },
      "scope-old",
      oldIndex
    );

    const data = writeMigrationCompletion(raw, selection, "explicit-scope", targetIndex);

    expect(readSyncIndex(data, "explicit-scope")).toEqual(targetIndex);
    expect(Object.keys(data.syncIndexes ?? {})).toEqual(["scope-old", "explicit-scope"]);
  });

  it("keeps unrelated settings written after the migration started", () => {
    const raw = writePluginSettingsPatch(
      {
        settings: {
          ...DEFAULT_SETTINGS,
          serverUrl: "https://sync.example.test",
          userId: "u1"
        }
      },
      { checkForUpdates: false, pollIntervalSeconds: 42 }
    );

    const data = writeMigrationCompletion(raw, selection, "scope-target", targetIndex);
    const settings = readPluginSettings(data);

    expect(settings.checkForUpdates).toBe(false);
    expect(settings.pollIntervalSeconds).toBe(42);
    expect(settings.selectedVaultId).toBe("vault-new");
  });
});
