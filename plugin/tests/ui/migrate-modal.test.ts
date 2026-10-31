import { describe, expect, it, vi } from "vitest";
import { en } from "../../src/i18n/en";
import { MigrateModal } from "../../src/ui/migrate-modal";
import type {
  MigrationProgress,
  MigrationResult
} from "../../src/sync/migrate-from-obsidian-sync";

function migrationResult(): MigrationResult {
  return {
    vaultId: "vault-1",
    vaultName: "Migrated vault",
    scannedFiles: 2,
    pushedFiles: 2,
    skippedCount: 1,
    totalBytes: 1536,
    batches: 1,
    lastCommit: "c1",
    index: {
      lastSyncedCommit: "c1",
      files: {}
    }
  };
}

function openedModal(
  onStart = vi.fn<
    (
      vaultName: string,
      onProgress: (progress: MigrationProgress) => void
    ) => Promise<MigrationResult>
  >().mockResolvedValue(migrationResult())
): {
  modal: MigrateModal;
  input: HTMLInputElement;
  start: HTMLButtonElement;
  progress: HTMLElement;
  inputListener: () => void;
  clickListener: () => void;
  onStart: typeof onStart;
} {
  const modal = new MigrateModal({} as never, en, {
    detection: {
      syncDirExists: true,
      syncPluginEnabled: false,
      likelyUsingSync: true
    },
    scan: {
      files: [
        { path: "a.md", size: 512 },
        { path: "b.md", size: 1024 }
      ],
      skippedCount: 1,
      totalBytes: 1536
    },
    initialVaultName: "Migrated vault",
    onStart
  });
  modal.open();

  const controls = modal as unknown as {
    vaultNameInput: HTMLInputElement | null;
    startButton: HTMLButtonElement | null;
    progressEl: HTMLElement | null;
  };
  const input = controls.vaultNameInput;
  const start = controls.startButton;
  const progress = controls.progressEl;
  if (!input || !start || !progress) throw new Error("modal controls were not rendered");

  const inputListener = vi
    .mocked(input.addEventListener)
    .mock.calls.find(([event]) => event === "input")?.[1] as (() => void) | undefined;
  const clickListener = vi
    .mocked(start.addEventListener)
    .mock.calls.find(([event]) => event === "click")?.[1] as (() => void) | undefined;
  if (!inputListener || !clickListener) throw new Error("modal listeners were not registered");

  return { modal, input, start, progress, inputListener, clickListener, onStart };
}

describe("MigrateModal", () => {
  it("keeps start disabled until a vault name is entered", () => {
    const { input, start, inputListener } = openedModal();

    input.value = "";
    inputListener();

    expect(start.disabled).toBe(true);
  });

  it("passes the trimmed vault name and renders progress and completion", async () => {
    const onStart = vi
      .fn<
        (
          vaultName: string,
          onProgress: (progress: MigrationProgress) => void,
          isCancelled?: () => boolean
        ) => Promise<MigrationResult>
      >()
      .mockImplementation(async (_vaultName, onProgress) => {
        onProgress({
          stage: "pushing",
          totalFiles: 2,
          processedFiles: 1,
          pushedFiles: 1,
          skippedCount: 1,
          totalBytes: 1536,
          uploadedBlobs: 0,
          totalBlobs: 0,
          currentBatch: 1,
          totalBatches: 1
        });
        return migrationResult();
      });
    const { input, progress, inputListener, clickListener } = openedModal(onStart);

    input.value = "  Migrated vault  ";
    inputListener();
    clickListener();
    await Promise.resolve();
    await Promise.resolve();

    expect(onStart).toHaveBeenCalledWith(
      "Migrated vault",
      expect.any(Function),
      expect.any(Function)
    );
    expect(progress.setText).toHaveBeenCalledWith(
      "1/2 files - batch 1/1 - blobs 0/0"
    );
    expect(progress.setText).toHaveBeenLastCalledWith("2 files migrated - 1 skipped");
  });

  it("cancels the completion path when the modal closes mid-migration (V5)", async () => {    let releaseMigration!: (value: MigrationResult) => void;
    const migrationGate = new Promise<MigrationResult>((resolve) => {
      releaseMigration = resolve;
    });
    let cancelledFlag = false;
    const onStart = vi
      .fn<
        (
          vaultName: string,
          onProgress: (progress: MigrationProgress) => void,
          isCancelled?: () => boolean
        ) => Promise<MigrationResult>
      >()
      .mockImplementation(async (_vaultName, _onProgress, isCancelled) => {
        // Simulate the modal being closed while batches upload.
        cancelledFlag = true;
        void isCancelled;
        return migrationGate;
      });
    const { modal, input, progress, inputListener, clickListener } =
      openedModal(onStart);

    input.value = "Migrated vault";
    inputListener();
    clickListener();

    // User closes the modal mid-run; the migration promise then resolves.
    modal.close();
    releaseMigration(migrationResult());
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    // The completion render must not fire: the status stays on the last
    // progress text rather than flipping to the completion summary.
    expect(progress.setText).not.toHaveBeenCalledWith(
      "2 files migrated - 1 skipped"
    );
    void cancelledFlag;
  });

  it("disables cancel and refuses to cancel once the commit stage starts", async () => {
    let release!: (value: MigrationResult) => void;
    const gate = new Promise<MigrationResult>((resolve) => {
      release = resolve;
    });
    let seenCancelled: boolean | null = null;
    const onStart = vi
      .fn<
        (
          vaultName: string,
          onProgress: (progress: MigrationProgress) => void,
          isCancelled?: () => boolean
        ) => Promise<MigrationResult>
      >()
      .mockImplementation(async (_vaultName, onProgress, isCancelled) => {
        onProgress({
          stage: "committing",
          totalFiles: 2,
          processedFiles: 2,
          pushedFiles: 2,
          skippedCount: 0,
          totalBytes: 1536,
          uploadedBlobs: 0,
          totalBlobs: 0,
          currentBatch: 1,
          totalBatches: 1
        });
        const result = await gate;
        seenCancelled = isCancelled ? isCancelled() : null;
        return result;
      });
    const { modal, input, progress, inputListener, clickListener } =
      openedModal(onStart);
    const controls = modal as unknown as {
      cancelButton: HTMLButtonElement | null;
      committing: boolean;
    };
    const cancel = controls.cancelButton;
    if (!cancel) throw new Error("cancel button was not captured");

    input.value = "Migrated vault";
    inputListener();
    clickListener();
    await Promise.resolve();
    await Promise.resolve();

    expect(controls.committing).toBe(true);
    expect(cancel.disabled).toBe(true);

    modal.close();
    release(migrationResult());
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    // The write already started, so closing must not pretend to cancel it.
    expect(seenCancelled).toBe(false);
    expect(progress.setText).not.toHaveBeenCalledWith(
      "2 files migrated - 0 skipped"
    );
  });
});
