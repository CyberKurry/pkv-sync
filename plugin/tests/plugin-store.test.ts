import { describe, expect, it } from "vitest";
import { writePluginSettings, writeSyncIndex } from "../src/plugin-data";
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
});
