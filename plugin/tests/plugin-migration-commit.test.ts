import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../src/api/client";
import { OperationStoppedError } from "../src/connection-context";
import { en } from "../src/i18n/en";
import { DataCommitError } from "../src/plugin-store";
import { notices } from "./mocks/obsidian";
import {
  createMigrationHarness,
  OLD_INDEX,
  targetScopeKey
} from "./helpers/migration-fixture";

interface CapturedOptions {
  onStart(
    vaultName: string,
    onProgress: (progress: { stage: string }) => void,
    isCancelled?: () => boolean
  ): Promise<unknown>;
}

const captured: CapturedOptions[] = [];

vi.mock("../src/ui/migrate-modal", () => ({
  MigrateModal: class {
    constructor(_app: unknown, _labels: unknown, options: CapturedOptions) {
      captured.push(options);
    }
    open(): void {}
  }
}));

const emptyIndex = { lastSyncedCommit: null, files: {} };

beforeEach(() => {
  captured.length = 0;
  notices.length = 0;
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("migration completion commit", () => {
  it("switches the vault and stores the target index in one record", async () => {
    const h = createMigrationHarness({
      files: [
        { path: "a.md", content: "alpha" },
        { path: "b.md", content: "beta" }
      ]
    });
    await h.plugin.openMigrationModal();
    const result = (await captured[0].onStart("New Vault", () => undefined, () => false)) as {
      vaultId: string;
      index: unknown;
    };

    expect(h.savedSettings().selectedVaultId).toBe("vault-new");
    expect(h.savedSettings().selectedVaultName).toBe("New Vault");
    expect(h.savedIndex(targetScopeKey(h.oldSettings(), "vault-new"))).toEqual(
      result.index
    );
    expect(h.savedIndex(h.oldScopeKey())).toEqual(OLD_INDEX);
    expect(notices).toContain(
      h.plugin.text().migrateCompleteNotice
        .replace("{count}", "2")
        .replace("{name}", "New Vault")
    );
    expect(h.events).toEqual(["invalidate", "rebuild"]);
  });

  it("keeps the old selection when the combined write is rejected", async () => {
    const h = createMigrationHarness({ files: [{ path: "a.md", content: "a" }] });
    h.setSave(async () => {
      throw new Error("disk full");
    });
    await h.plugin.openMigrationModal();

    const error = await captured[0]
      .onStart("New Vault", () => undefined, () => false)
      .catch((reason: unknown) => reason);

    expect(error).toBeInstanceOf(DataCommitError);
    expect((error as DataCommitError).state).toBe("restored");
    expect(h.savedSettings().selectedVaultId).toBe("vault-old");
    expect(h.savedIndex(targetScopeKey(h.oldSettings(), "vault-new"))).toEqual(
      emptyIndex
    );
    expect(h.events).toEqual(["invalidate", "rebuild"]);
    expect(notices).toContain(h.plugin.text().migrateSaveFailed);
    expect(h.plugin.persistenceRecoveryRequired).toBe(false);
  });

  it("rolls back a write that landed and then failed, before restoring the engine", async () => {
    const h = createMigrationHarness({ files: [{ path: "a.md", content: "a" }] });
    let writes = 0;
    h.setSave(async (next) => {
      writes += 1;
      h.setDisk(next);
      if (writes === 1) throw new Error("flush failed");
    });
    await h.plugin.openMigrationModal();

    const error = await captured[0]
      .onStart("New Vault", () => undefined, () => false)
      .catch((reason: unknown) => reason);

    expect((error as DataCommitError).state).toBe("restored");
    expect(writes).toBe(2);
    expect(h.savedSettings().selectedVaultId).toBe("vault-old");
    expect(h.savedIndex(h.oldScopeKey())).toEqual(OLD_INDEX);
    expect(h.events).toEqual(["invalidate", "rebuild"]);
    expect(notices).toContain(h.plugin.text().migrateSaveFailed);
  });

  it("pauses the plugin when neither the read back nor the compensation works", async () => {
    const h = createMigrationHarness({ files: [{ path: "a.md", content: "a" }] });
    h.setSave(async (next) => {
      h.setDisk(next);
      throw new Error("save failed");
    });
    let loads = 0;
    h.setLoad(async () => {
      loads += 1;
      if (loads > 1) throw new Error("read back failed");
      return h.disk();
    });
    await h.plugin.openMigrationModal();

    const error = await captured[0]
      .onStart("New Vault", () => undefined, () => false)
      .catch((reason: unknown) => reason);

    expect((error as DataCommitError).state).toBe("uncertain");
    expect(h.plugin.persistenceRecoveryRequired).toBe(true);
    expect(h.events).toEqual(["invalidate", "invalidate"]);
    expect(notices).toContain(h.plugin.text().migrateRecoveryFailed);
    expect(notices).not.toContain(h.plugin.text().migrateCompleteNotice);

    h.setLoad(null);
    h.setSave(async (next) => {
      h.setDisk(next);
    });
    await h.plugin.resumeSyncForCurrentScope();
    await h.plugin.setConnectionPatch({ serverUrl: "https://elsewhere.test" });
    expect(h.events).not.toContain("rebuild");
  });

  it("does not save anything when the connection changed before the commit ran", async () => {
    const h = createMigrationHarness({ files: [{ path: "a.md", content: "a" }] });
    await h.plugin.openMigrationModal();
    let switching: Promise<unknown> | null = null;
    const onProgress = (progress: { stage: string }) => {
      if (progress.stage !== "pushing" || switching) return;
      switching = h.plugin.setConnectionPatch({
        serverUrl: "https://new.example.test"
      });
    };

    const error = await captured[0]
      .onStart("New Vault", onProgress, () => false)
      .catch((reason: unknown) => reason);
    await switching;

    expect(error).toBeInstanceOf(Error);
    expect((error as Error).message).toBe(h.plugin.text().migrateScopeChanged);
    expect(h.savedSettings().selectedVaultId).toBe("vault-old");
    expect(h.savedIndex(targetScopeKey(h.oldSettings(), "vault-new"))).toEqual(
      emptyIndex
    );
    expect(h.savedSettings().serverUrl).toBe("");
    expect(h.events).toEqual(["invalidate", "invalidate"]);
  });

  it("keeps the committed record when the engine cannot be rebuilt", async () => {
    const h = createMigrationHarness({ files: [{ path: "a.md", content: "a" }] });
    (
      h.plugin as unknown as { orchestratorInstance: { rebuild(): void } }
    ).orchestratorInstance.rebuild = () => {
      throw new Error("engine boom");
    };
    await h.plugin.openMigrationModal();

    await expect(captured[0].onStart(
      "New Vault",
      () => undefined,
      () => false
    )).rejects.toThrow(h.plugin.text().migrateEngineFailed);

    expect(h.savedSettings().selectedVaultId).toBe("vault-new");
    expect(h.savedIndex(targetScopeKey(h.oldSettings(), "vault-new")).files).toHaveProperty("a.md");
    expect(notices).toContain(h.plugin.text().migrateEngineFailed);
    expect(notices).not.toContain(h.plugin.text().migrateRecoveryFailed);
  });

  it("does not rebuild or claim the newer scope when the migration was superseded", async () => {
    const h = createMigrationHarness({ files: [{ path: "a.md", content: "a" }] });
    await h.plugin.openMigrationModal();
    let switched: Promise<boolean> | null = null;
    const onProgress = (progress: { stage: string }) => {
      if (progress.stage !== "pushing" || switched) return;
      switched = h.plugin.setSelectedVault("vault-other", "Other");
    };

    const error = await captured[0]
      .onStart("First Vault", onProgress, () => false)
      .catch((reason: unknown) => reason);
    await switched;

    expect(error).toBeInstanceOf(Error);
    expect((error as Error).message).toBe(h.plugin.text().migrateScopeChanged);
    expect(h.savedSettings().selectedVaultId).toBe("vault-other");
    expect(h.events).toEqual(["invalidate"]);
    expect(h.savedIndex(targetScopeKey(h.oldSettings(), "vault-new"))).toEqual(
      emptyIndex
    );
  });

  it("keeps the new selection and index when a full settings save follows", async () => {
    const h = createMigrationHarness({ files: [{ path: "a.md", content: "a" }] });
    await h.plugin.openMigrationModal();
    const running = captured[0].onStart(
      "New Vault",
      () => undefined,
      () => false
    );
    await running;
    await h.plugin.saveSettings({ rebuild: false });

    expect(h.savedSettings().selectedVaultId).toBe("vault-new");
    expect(h.savedSettings().selectedVaultName).toBe("New Vault");
    expect(h.savedIndex(h.oldScopeKey())).toEqual(OLD_INDEX);
    expect(
      h.savedIndex(targetScopeKey(h.oldSettings(), "vault-new")).lastSyncedCommit
    ).toBe("c1");
  });

  it("writes nothing when the migration is cancelled before the commit", async () => {
    const h = createMigrationHarness({ files: [{ path: "a.md", content: "a" }] });
    await h.plugin.openMigrationModal();
    let cancel = false;
    const onProgress = (progress: { stage: string }) => {
      if (progress.stage === "pushing") cancel = true;
    };

    const error = await captured[0]
      .onStart("New Vault", onProgress, () => cancel)
      .catch((reason: unknown) => reason);

    expect(error).toBeInstanceOf(OperationStoppedError);
    expect((error as OperationStoppedError).reason).toBe("cancelled");
    expect(h.savedSettings().selectedVaultId).toBe("vault-old");
    expect(h.saveCalls()).toBe(0);
    expect(h.events).toEqual(["invalidate", "rebuild"]);
  });
});
