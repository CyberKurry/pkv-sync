import { afterEach, describe, expect, it, vi } from "vitest";
import { en } from "../src/i18n/en";

/**
 * V3: a failed migration must still rebuild the sync engine.
 * openMigrationModal invalidates the engine BEFORE migrateToPkv runs;
 * without an explicit rebuild on the failure path, the engine stayed null
 * for the rest of the session — the status bar kept saying "connected"
 * while every file change silently no-oped.
 */

interface CapturedOptions {
  onStart(
    vaultName: string,
    onProgress: (p: unknown) => void
  ): Promise<unknown>;
}

const captured: CapturedOptions[] = [];

vi.mock("../src/ui/migrate-modal", () => ({
  MigrateModal: class {
    constructor(
      _app: unknown,
      _labels: unknown,
      options: CapturedOptions
    ) {
      captured.push(options);
    }
    open(): void {}
  }
}));

vi.mock("../src/sync/migrate-from-obsidian-sync", () => ({
  scanVaultForMigration: () => ({
    files: [],
    skippedCount: 0,
    totalBytes: 0
  }),
  detectObsidianSync: async () => ({
    syncDirExists: false,
    syncPluginEnabled: false,
    likelyUsingSync: false
  }),
  migrateToPkv: vi.fn()
}));

import PKVSyncPlugin from "../src/main";
import { migrateToPkv } from "../src/sync/migrate-from-obsidian-sync";

type MigrationHarness = {
  app: { vault: { getName?(): string } };
  settings: Record<string, unknown>;
  manifest: { version: string };
  api(): unknown;
  text(): Record<string, string>;
  invalidateSyncEngine(): void;
  rebuildSyncEngine(): void;
  saveSettings(opts?: { rebuild?: boolean }): Promise<void>;
  saveSyncIndex(index: unknown): Promise<void>;
  openMigrationModal(): Promise<void>;
};

function makePlugin(): { plugin: MigrationHarness; calls: string[] } {
  const calls: string[] = [];
  const plugin = Object.create(PKVSyncPlugin.prototype) as MigrationHarness;
  plugin.app = { vault: { getName: () => "My Vault" } };
  plugin.settings = {
    deviceName: "Laptop",
    textExtensions: ["md"],
    serverUrl: "https://sync.example.com",
    deploymentKey: "k_x",
    token: "tok"
  };
  plugin.manifest = { version: "1.7.0" };
  plugin.api = vi.fn(() => ({}));
  plugin.invalidateSyncEngine = () => calls.push("invalidate");
  plugin.rebuildSyncEngine = () => calls.push("rebuild");
  plugin.saveSettings = vi.fn(async () => {
    calls.push("saveSettings");
  });
  plugin.saveSyncIndex = vi.fn(async () => {
    calls.push("saveSyncIndex");
  });
  return { plugin, calls };
}

describe("PKVSyncPlugin migration failure (V3)", () => {
  afterEach(() => {
    captured.length = 0;
    vi.mocked(migrateToPkv).mockReset();
  });

  it("rebuilds the engine when migrateToPkv throws", async () => {
    vi.mocked(migrateToPkv).mockRejectedValue(
      new Error("429 too many requests")
    );
    const { plugin, calls } = makePlugin();
    plugin.text = () => en as unknown as Record<string, string>;

    await plugin.openMigrationModal();
    expect(captured).toHaveLength(1);

    await expect(
      captured[0].onStart("Migrated vault", () => undefined)
    ).rejects.toThrow("429");
    // The engine was invalidated before the migration; the failure path
    // must rebuild it instead of leaving it null for the session.
    expect(calls).toEqual(["invalidate", "rebuild"]);
    // And it must NOT have switched vaults or claimed success.
    expect(calls).not.toContain("saveSettings");
    expect(calls).not.toContain("saveSyncIndex");
    expect(plugin.settings.selectedVaultId).toBeUndefined();
  });

  it("keeps the success path unchanged: rebuild happens exactly once after completion", async () => {
    vi.mocked(migrateToPkv).mockResolvedValue({
      vaultId: "v1",
      vaultName: "Migrated",
      scannedFiles: 3,
      pushedFiles: 3,
      skippedCount: 0,
      totalBytes: 100,
      batches: 1,
      lastCommit: "c1",
      index: { lastSyncedCommit: "c1", files: {} }
    });
    const { plugin, calls } = makePlugin();
    plugin.text = () => en as unknown as Record<string, string>;

    await plugin.openMigrationModal();
    await captured[0].onStart("Migrated vault", () => undefined);

    expect(calls).toEqual(["invalidate", "saveSettings", "saveSyncIndex", "rebuild"]);
  });
});
