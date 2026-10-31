import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { OperationStoppedError } from "../src/connection-context";
import { notices } from "./mocks/obsidian";
import { createMigrationHarness, OLD_INDEX, targetScopeKey } from "./helpers/migration-fixture";

/**
 * V3: a failed migration must still rebuild the sync engine.
 * V5: closing the modal mid-migration must not switch the vault.
 *
 * These invariants used to be asserted through the ORDER of saveSettings /
 * saveSyncIndex calls, which proved nothing about what actually reached the
 * disk. The assertions below read the persisted record instead.
 */

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

beforeEach(() => {
  captured.length = 0;
  notices.length = 0;
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("migration failure recovery", () => {
  it("rebuilds the engine and keeps the old record when the migration fails", async () => {
    const h = createMigrationHarness({ files: [{ path: "a.md", content: "a" }] });
    h.failCreateVault(new Error("429 too many requests"));
    await h.plugin.openMigrationModal();

    await expect(
      captured[0].onStart("Migrated vault", () => undefined, () => false)
    ).rejects.toThrow("429");

    expect(h.savedSettings().selectedVaultId).toBe("vault-old");
    expect(h.savedSettings().lastSyncSuccessAt).toBeNull();
    expect(h.savedIndex(h.oldScopeKey())).toEqual(OLD_INDEX);
    expect(h.savedIndex(targetScopeKey(h.oldSettings(), "vault-new"))).toEqual({
      lastSyncedCommit: null,
      files: {}
    });
    expect(h.events).toEqual(["invalidate", "rebuild"]);
    expect(notices).toEqual([]);
  });

  it("does not switch vaults when the migration was cancelled mid-run", async () => {
    const h = createMigrationHarness({ files: [{ path: "a.md", content: "a" }] });
    await h.plugin.openMigrationModal();

    const error = await captured[0]
      .onStart("Migrated vault", () => undefined, () => true)
      .catch((reason: unknown) => reason);

    expect(error).toBeInstanceOf(OperationStoppedError);
    expect((error as OperationStoppedError).reason).toBe("cancelled");
    expect(h.savedSettings().selectedVaultId).toBe("vault-old");
    expect(h.savedIndex(h.oldScopeKey())).toEqual(OLD_INDEX);
    expect(h.events).toEqual(["invalidate", "rebuild"]);
    expect(notices).toEqual([]);
  });

  it("reports the scope change in the user's language, not the machine reason", async () => {
    const h = createMigrationHarness({ files: [{ path: "a.md", content: "a" }] });
    await h.plugin.openMigrationModal();
    let switched: Promise<unknown> | null = null;
    const onProgress = (progress: { stage: string }) => {
      if (progress.stage !== "pushing" || switched) return;
      switched = h.plugin.setConnectionPatch({
        serverUrl: "https://switched.example.test"
      });
    };

    const error = await captured[0]
      .onStart("Migrated vault", onProgress, () => false)
      .catch((reason: unknown) => reason);
    await switched;

    expect(error).toBeInstanceOf(Error);
    expect((error as Error).message).toBe(
      h.plugin.text().migrateScopeChanged
    );
    expect(notices).toContain(h.plugin.text().migrateScopeChanged);
  });
});
