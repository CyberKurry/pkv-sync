import { describe, expect, it, vi } from "vitest";
import { OperationStoppedError } from "../src/connection-context";
import {
  readPluginSettings,
  readSyncIndex,
  writePluginSettingsWithoutAuth,
  writePluginSettingsPatch,
  writeSyncIndex
} from "../src/plugin-data";
import { DataCommitError, SerializedPluginDataStore } from "../src/plugin-store";
import { DEFAULT_SETTINGS, type PKVSyncSettings } from "../src/settings";
import { writeMigrationCompletion } from "../src/sync/migration-commit";
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
    const second = store.update((raw) =>
      writePluginSettingsWithoutAuth(raw, nextSettings)
    );

    await Promise.resolve();
    gate.resolve();
    await Promise.all([first, second]);

    expect(stored).toEqual({
      settings: settingsWithoutAuth(nextSettings),
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
      return writePluginSettingsWithoutAuth(raw, nextSettings);
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

    expect(stored).toEqual({
      settings: {
        ...settingsWithoutAuth(nextSettings),
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

function settingsWithoutAuth(settings: PKVSyncSettings): Partial<PKVSyncSettings> {
  const {
    deviceId: _deviceId,
    token: _token,
    serverUrl: _serverUrl,
    deploymentKey: _deploymentKey,
    userId: _userId,
    ...rest
  } = settings;
  return rest;
}

const selection = {
  selectedVaultId: "new",
  selectedVaultName: "New",
  lastSyncSuccessAt: 123
};

const targetIndex: LocalIndex = { lastSyncedCommit: "c1", files: {} };
const oldIndex: LocalIndex = { lastSyncedCommit: "old", files: {} };

function diskWith(selectedVaultId: string): unknown {
  return {
    settings: { ...DEFAULT_SETTINGS, selectedVaultId },
    syncIndexes: { "scope-old": oldIndex }
  };
}

function settle(promise: Promise<unknown>): Promise<unknown> {
  return promise.then(
    () => {
      throw new Error("expected the commit to fail");
    },
    (reason: unknown) => reason
  );
}

describe("SerializedPluginDataStore.commitWithRecovery", () => {
  it("reports a restored state when the save rejects without writing", async () => {
    const disk = diskWith("old");
    const save = vi.fn(async () => {
      throw new Error("disk full");
    });
    const store = new SerializedPluginDataStore(
      async () => structuredClone(disk),
      save
    );
    const publish = vi.fn(() => undefined);

    const error = await settle(
      store.commitWithRecovery(
        (raw) => writeMigrationCompletion(raw, selection, "target", targetIndex),
        { beforeSave: () => undefined, publish }
      )
    );

    expect(error).toBeInstanceOf(DataCommitError);
    expect((error as DataCommitError).state).toBe("restored");
    expect(save).toHaveBeenCalledTimes(1);
    expect(publish).not.toHaveBeenCalled();
    expect(readPluginSettings(disk).selectedVaultId).toBe("old");
  });

  it("restores the previous record when the save writes and then rejects", async () => {
    let disk = diskWith("old");
    let calls = 0;
    const store = new SerializedPluginDataStore(
      async () => structuredClone(disk),
      async (next) => {
        calls += 1;
        disk = structuredClone(next);
        if (calls === 1) throw new Error("flush failed");
      }
    );
    const queuedEvents: string[] = [];

    const commit = store.commitWithRecovery(
      (raw) => writeMigrationCompletion(raw, selection, "target", targetIndex),
      { beforeSave: () => undefined, publish: () => undefined }
    );
    const queued = store.update((raw) => {
      queuedEvents.push("queued");
      return writePluginSettingsPatch(raw, { debounceMs: 900 });
    });

    const error = await settle(commit);
    expect((error as DataCommitError).state).toBe("restored");
    expect(calls).toBe(2);
    expect(readPluginSettings(disk).selectedVaultId).toBe("old");
    expect(readSyncIndex(disk, "target")).toEqual({
      lastSyncedCommit: null,
      files: {}
    });

    await queued;
    expect(queuedEvents).toEqual(["queued"]);
    expect(readPluginSettings(disk).selectedVaultId).toBe("old");
    expect(readPluginSettings(disk).debounceMs).toBe(900);
  });

  it("reports an uncertain state when neither the read back nor the compensation works", async () => {
    let loads = 0;
    const store = new SerializedPluginDataStore(
      async () => {
        loads += 1;
        if (loads > 1) throw new Error("read back failed");
        return diskWith("old");
      },
      async () => {
        throw new Error("save failed");
      }
    );

    const error = await settle(
      store.commitWithRecovery(
        (raw) => writeMigrationCompletion(raw, selection, "target", targetIndex),
        { beforeSave: () => undefined, publish: () => undefined }
      )
    );

    expect((error as DataCommitError).state).toBe("uncertain");
    expect((error as DataCommitError).recovery).toBeInstanceOf(Error);
  });

  it("does not roll back a committed record when the publish step throws", async () => {
    let disk = diskWith("old");
    const store = new SerializedPluginDataStore(
      async () => structuredClone(disk),
      async (next) => {
        disk = structuredClone(next);
      }
    );

    const error = await settle(
      store.commitWithRecovery(
        (raw) => writeMigrationCompletion(raw, selection, "target", targetIndex),
        {
          beforeSave: () => undefined,
          publish: () => {
            throw new Error("render failed");
          }
        }
      )
    );

    expect((error as DataCommitError).state).toBe("committed");
    expect(readPluginSettings(disk).selectedVaultId).toBe("new");
    expect(readSyncIndex(disk, "target")).toEqual(targetIndex);
  });

  it("skips the save entirely when the pre-save check throws", async () => {
    const save = vi.fn(async () => undefined);
    const store = new SerializedPluginDataStore(
      async () => diskWith("old"),
      save
    );

    const error = await settle(
      store.commitWithRecovery(
        (raw) => writeMigrationCompletion(raw, selection, "target", targetIndex),
        {
          beforeSave: () => {
            throw new OperationStoppedError("cancelled");
          },
          publish: () => undefined
        }
      )
    );

    expect(error).toBeInstanceOf(OperationStoppedError);
    expect(save).not.toHaveBeenCalled();
  });

  it("publishes selection before the following queued full save reads settings", async () => {
    let settings: PKVSyncSettings = { ...DEFAULT_SETTINGS, selectedVaultId: "old" };
    let disk: unknown = { settings, syncIndexes: {} };
    const store = new SerializedPluginDataStore(
      async () => structuredClone(disk),
      async (next) => {
        disk = structuredClone(next);
      }
    );
    const index = { lastSyncedCommit: "c1", files: {} } as LocalIndex;
    const first = store.commitWithRecovery(
      (raw) => writeMigrationCompletion(raw, selection, "target-scope", index),
      {
        beforeSave: () => undefined,
        publish: () => {
          settings = { ...settings, ...selection };
          return undefined;
        }
      }
    );
    const next = store.update((raw) => writePluginSettingsWithoutAuth(raw, settings));
    await Promise.all([first, next]);
    expect(readPluginSettings(disk).selectedVaultId).toBe("new");
    expect(readSyncIndex(disk, "target-scope")).toEqual(index);
  });
});
