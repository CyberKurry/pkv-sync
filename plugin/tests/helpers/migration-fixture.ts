import { TFile } from "obsidian";
import type { ApiClient } from "../../src/api/client";
import PKVSyncPlugin from "../../src/main";
import { readPluginSettings, readSyncIndex, syncScopeKey } from "../../src/plugin-data";
import { SerializedPluginDataStore } from "../../src/plugin-store";
import { DEFAULT_SETTINGS, type PKVSyncSettings } from "../../src/settings";
import type { LocalIndex } from "../../src/sync/types";

export interface MigrationFileSeed {
  path: string;
  content?: string;
  bytes?: Uint8Array;
}

class MigrationFakeVault {
  configDir = ".obsidian";
  private readonly entries = new Map<string, TFile>();
  private readonly text = new Map<string, string>();
  private readonly binary = new Map<string, ArrayBuffer>();

  constructor(files: MigrationFileSeed[]) {
    for (const file of files) {
      const entry = Object.create(TFile.prototype) as TFile;
      if (file.bytes) {
        const copy = new ArrayBuffer(file.bytes.byteLength);
        new Uint8Array(copy).set(file.bytes);
        Object.assign(entry, { path: file.path, stat: { size: file.bytes.byteLength } });
        this.binary.set(file.path, copy);
      } else {
        const content = file.content ?? "";
        Object.assign(entry, {
          path: file.path,
          stat: { size: new TextEncoder().encode(content).byteLength }
        });
        this.text.set(file.path, content);
      }
      this.entries.set(file.path, entry);
    }
  }

  getName(): string {
    return "My Vault";
  }

  getFiles(): TFile[] {
    return [...this.entries.values()];
  }

  getAbstractFileByPath(path: string): TFile | null {
    return this.entries.get(path) ?? null;
  }

  async read(file: TFile): Promise<string> {
    const content = this.text.get(file.path);
    if (content === undefined) throw new Error(`Unreadable file: ${file.path}`);
    return content;
  }

  async readBinary(file: TFile): Promise<ArrayBuffer> {
    const bytes = this.binary.get(file.path);
    if (bytes === undefined) throw new Error(`Unreadable binary: ${file.path}`);
    return bytes.slice(0);
  }
}

export interface MigrationHarnessOptions {
  files?: MigrationFileSeed[];
  settings?: Partial<PKVSyncSettings>;
}

export interface MigrationHarness {
  plugin: PKVSyncPlugin;
  events: string[];
  disk(): unknown;
  setDisk(next: unknown): void;
  saveCalls(): number;
  setSave(fn: (next: unknown) => Promise<void>): void;
  setLoad(fn: (() => Promise<unknown>) | null): void;
  failCreateVault(error: Error | null): void;
  failPush(error: Error | null): void;
  holdNextSave(): { release(): void };
  oldSettings(): PKVSyncSettings;
  oldScopeKey(): string;
  savedSettings(): PKVSyncSettings;
  savedIndex(scope: string): LocalIndex;
}

const oldIndex: LocalIndex = {
  lastSyncedCommit: "old-head",
  files: {
    "existing.md": {
      lastSyncedHash: "hash-existing",
      lastSyncedAt: 1,
      kind: "text",
      size: 4
    }
  }
};

export function createMigrationHarness(
  options: MigrationHarnessOptions = {}
): MigrationHarness {
  const events: string[] = [];
  const settings: PKVSyncSettings = {
    ...DEFAULT_SETTINGS,
    serverUrl: "https://old.example.test",
    deploymentKey: "k-old",
    token: "token-old",
    userId: "user-old",
    username: "old",
    selectedVaultId: "vault-old",
    selectedVaultName: "Old Vault",
    deviceName: "Laptop",
    textExtensions: ["md"],
    ...options.settings
  };
  const initialDisk = {
    settings: { ...settings },
    syncIndexes: { [syncScopeKey(settings)]: oldIndex }
  };
  const originalSettings: PKVSyncSettings = {
    ...settings,
    textExtensions: [...settings.textExtensions]
  };
  let disk: unknown = structuredClone(initialDisk);

  const plugin = Object.create(PKVSyncPlugin.prototype) as PKVSyncPlugin;
  plugin.settings = settings;
  plugin.manifest = {
    id: "pkv-sync",
    version: "1.7.1",
    dir: "plugins/pkv-sync"
  } as PKVSyncPlugin["manifest"];
  plugin.serverPushLimits = { max_json_bytes: 2_097_152, max_changes: 1000 };
  plugin.persistenceRecoveryRequired = false;
  (
    plugin as unknown as { app: unknown }
  ).app = { vault: new MigrationFakeVault(options.files ?? []) };
  (
    plugin as unknown as { authStore: { save(value: unknown): void } }
  ).authStore = { save: () => undefined };
  (
    plugin as unknown as {
      orchestratorInstance: {
        rebuild(): void;
        invalidate(): void;
        syncNow(): Promise<void>;
      };
    }
  ).orchestratorInstance = {
    rebuild: () => events.push("rebuild"),
    invalidate: () => events.push("invalidate"),
    syncNow: async () => undefined
  };

  let saveImpl: (next: unknown) => Promise<void> = async (next) => {
    disk = structuredClone(next);
  };
  let loadOverride: (() => Promise<unknown>) | null = null;
  let saveCount = 0;
  let heldSave: (() => Promise<void>) | null = null;

  (
    plugin as unknown as { dataStore: SerializedPluginDataStore }
  ).dataStore = new SerializedPluginDataStore(
    async () => (loadOverride ? loadOverride() : structuredClone(disk)),
    async (next) => {
      saveCount += 1;
      const hold = heldSave;
      heldSave = null;
      if (hold) await hold();
      await saveImpl(next);
    }
  );

  let createVaultError: Error | null = null;
  let pushError: Error | null = null;
  let commit = 0;
  const client = {
    async createVault(name: string) {
      if (createVaultError) throw createVaultError;
      return {
        id: "vault-new",
        user_id: "user-old",
        name,
        created_at: 1,
        last_sync_at: null,
        size_bytes: 0,
        file_count: 0
      };
    },
    async request(_method: string, path: string, body?: unknown) {
      if (path.endsWith("/state")) {
        return { current_head: null, changed_since: false };
      }
      if (path.endsWith("/upload/check")) {
        return { missing: [] };
      }
      if (path.endsWith("/push")) {
        if (pushError) throw pushError;
        const changes = (body as { changes: unknown[] }).changes;
        commit += 1;
        return { new_commit: `c${commit}`, files_changed: changes.length };
      }
      throw new Error(`unexpected request: ${path}`);
    }
  } as unknown as ApiClient;
  plugin.apiForConnection = () => client;

  return {
    plugin,
    events,
    disk: () => disk,
    setDisk(next) {
      disk = next;
    },
    saveCalls: () => saveCount,
    setSave(fn) {
      saveImpl = fn;
    },
    setLoad(fn) {
      loadOverride = fn;
    },
    failCreateVault(error) {
      createVaultError = error;
    },
    failPush(error) {
      pushError = error;
    },
    holdNextSave() {
      let release!: () => void;
      const gate = new Promise<void>((resolve) => {
        release = resolve;
      });
      heldSave = () => gate;
      return { release };
    },
    oldSettings: () => ({
      ...originalSettings,
      textExtensions: [...originalSettings.textExtensions]
    }),
    oldScopeKey: () => syncScopeKey(originalSettings),
    savedSettings: () => readPluginSettings(disk),
    savedIndex: (scope) => readSyncIndex(disk, scope)
  };
}

export const OLD_INDEX = oldIndex;

export function targetScopeKey(settings: PKVSyncSettings, vaultId: string): string {
  return syncScopeKey({ ...settings, selectedVaultId: vaultId });
}
