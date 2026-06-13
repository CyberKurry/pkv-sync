import { describe, expect, it } from "vitest";
import {
  writePluginSettings,
  writePluginSettingsPatch,
  writeSyncIndex
} from "../src/plugin-data";
import { SerializedPluginDataStore } from "../src/plugin-store";
import { DEFAULT_SETTINGS } from "../src/settings";
import type { LocalIndex } from "../src/sync/types";

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

describe("SerializedPluginDataStore", () => {
  it("serializes load-modify-save updates so concurrent writes do not drop data", async () => {
    let stored: unknown = { settings: DEFAULT_SETTINGS };
    const gate = deferred();
    const index: LocalIndex = {
      lastSyncedCommit: "c1",
      files: {}
    };
    const nextSettings = {
      ...DEFAULT_SETTINGS,
      serverUrl: "https://sync.example.test"
    };
    const store = new SerializedPluginDataStore(
      async () => stored,
      async (data) => {
        stored = data;
      }
    );

    const first = store.update(async (raw) => {
      await gate.promise;
      return writeSyncIndex(raw, "scope-a", index);
    });
    const second = store.update((raw) => writePluginSettings(raw, nextSettings));

    await Promise.resolve();
    gate.resolve();
    await Promise.all([first, second]);

    expect(stored).toEqual({
      settings: nextSettings,
      syncIndexes: { "scope-a": index }
    });
  });

  it("merges concurrent partial settings updates without clobbering full saves", async () => {
    let stored: unknown = { settings: DEFAULT_SETTINGS };
    const gate = deferred();
    const nextSettings = {
      ...DEFAULT_SETTINGS,
      serverUrl: "https://sync.example.test",
      debounceMs: 250,
      lastUpdateCheckAt: null
    };
    const store = new SerializedPluginDataStore(
      async () => stored,
      async (data) => {
        stored = data;
      }
    );

    const fullSave = store.update(async (raw) => {
      await gate.promise;
      return writePluginSettings(raw, nextSettings);
    });
    const updateCheckWrite = store.update((raw) =>
      writePluginSettingsPatch(raw, { lastUpdateCheckAt: 1_700_000_000 })
    );
    const debounceWrite = store.update((raw) =>
      writePluginSettingsPatch(raw, { debounceMs: 1_000 })
    );

    await Promise.resolve();
    gate.resolve();
    await Promise.all([fullSave, updateCheckWrite, debounceWrite]);

    const {
      deviceId: _d,
      token: _t,
      serverUrl: _s,
      deploymentKey: _dk,
      userId: _u,
      ...nextSettingsWithoutAuth
    } = nextSettings;
    expect(stored).toEqual({
      settings: {
        ...nextSettingsWithoutAuth,
        lastUpdateCheckAt: 1_700_000_000,
        debounceMs: 1_000
      }
    });
  });

  it("waits for queued writes before reading data", async () => {
    let stored: unknown = { syncIndexes: {} };
    const gate = deferred();
    const index: LocalIndex = {
      lastSyncedCommit: "c1",
      files: {}
    };
    const store = new SerializedPluginDataStore(
      async () => stored,
      async (data) => {
        stored = data;
      }
    );

    const write = store.update(async (raw) => {
      await gate.promise;
      return writeSyncIndex(raw, "scope-a", index);
    });
    const read = store.read((raw) => raw);

    await Promise.resolve();
    gate.resolve();

    await expect(read).resolves.toEqual({
      syncIndexes: { "scope-a": index }
    });
    await write;
  });

  it("rejects accidental undefined update results", async () => {
    let stored: unknown = { settings: DEFAULT_SETTINGS };
    const store = new SerializedPluginDataStore(
      async () => stored,
      async (data) => {
        stored = data;
      }
    );

    await expect(store.update(() => undefined)).rejects.toThrow(
      "Plugin data update returned undefined"
    );
    expect(stored).toEqual({ settings: DEFAULT_SETTINGS });
  });
});
