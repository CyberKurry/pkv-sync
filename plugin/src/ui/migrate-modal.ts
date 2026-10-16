import { type App, Modal, Notice } from "obsidian";
import { formatBytes } from "../format";
import { format, type Strings } from "../i18n";
import { errorToMessage } from "../util";
import type {
  MigrationProgress,
  MigrationResult,
  MigrationScan,
  ObsidianSyncDetection
} from "../sync/migrate-from-obsidian-sync";

export interface MigrateModalOptions {
  detection: ObsidianSyncDetection;
  scan: MigrationScan;
  initialVaultName: string;
  onStart(
    vaultName: string,
    onProgress: (progress: MigrationProgress) => void,
    isCancelled?: () => boolean
  ): Promise<MigrationResult>;
}

export class MigrateModal extends Modal {
  private vaultNameInput: HTMLInputElement | null = null;
  private startButton: HTMLButtonElement | null = null;
  private cancelButton: HTMLButtonElement | null = null;
  private statusEl: HTMLElement | null = null;
  private progressEl: HTMLElement | null = null;
  private running = false;
  /** V5: closing the modal mid-migration cancels the post-migration
   * steps. The upload batches themselves are not abortable, but the
   * vault switch, index save, and "migrated" notice must not fire for a
   * migration the user walked away from. */
  private cancelled = false;

  constructor(
    app: App,
    private labels: Strings,
    private options: MigrateModalOptions
  ) {
    super(app);
  }

  onOpen(): void {
    this.renderReady();
  }

  onClose(): void {
    // V5: a close during a running migration marks it cancelled so the
    // completion path no-ops instead of switching the vault behind the
    // user's back.
    if (this.running) this.cancelled = true;
    this.contentEl.empty();
  }

  private renderReady(): void {
    this.contentEl.empty();
    this.contentEl.addClass("pkvsync-migrate-modal");
    this.contentEl.createEl("h2", { text: this.labels.migrateModalTitle });

    this.statusEl = this.contentEl.createDiv({
      cls: "pkvsync-migrate-status",
      text: this.options.detection.likelyUsingSync
        ? this.labels.migrateSyncDetected
        : this.labels.migrateSyncNotDetected
    });
    this.contentEl.createDiv({
      cls: "pkvsync-migrate-scan",
      text: format(this.labels.migrateScanSummary, {
        count: this.options.scan.files.length,
        size: formatBytes(this.options.scan.totalBytes),
        skipped: this.options.scan.skippedCount
      })
    });
    this.contentEl.createDiv({
      cls: "pkvsync-migrate-notice",
      text: this.labels.migrateHistoryNotice
    });

    const field = this.contentEl.createDiv({ cls: "pkv-sync-field" });
    field.createEl("label", {
      cls: "pkv-sync-label",
      text: this.labels.migrateVaultNameLabel
    });
    this.vaultNameInput = field.createEl("input", {
      cls: "pkv-sync-input",
      value: this.options.initialVaultName
    });
    this.vaultNameInput.value = this.options.initialVaultName;
    this.vaultNameInput.addEventListener("input", () => this.updateStartState());

    this.progressEl = this.contentEl.createDiv({ cls: "pkvsync-migrate-progress" });

    const actions = this.contentEl.createDiv({ cls: "pkv-sync-button-row" });
    const cancelButton = actions.createEl("button", {
      cls: "pkv-sync-button is-secondary",
      text: this.labels.migrateCancelButton
    });
    cancelButton.addEventListener("click", () => this.close());
    this.startButton = actions.createEl("button", {
      cls: "pkv-sync-button is-primary",
      text: this.labels.migrateStartButton
    });
    this.startButton.addEventListener("click", () => void this.start());
    this.updateStartState();
  }

  private async start(): Promise<void> {
    const vaultName = this.vaultNameInput?.value?.trim() ?? "";
    if (!vaultName) {
      new Notice(this.labels.migrateVaultNameRequired);
      return;
    }
    this.running = true;
    this.cancelled = false;
    this.updateStartState();
    try {
      const result = await this.options.onStart(
        vaultName,
        (progress) => this.renderProgress(progress),
        // V5: lets onStart skip its post-migration steps (vault switch,
        // index save, "migrated" notice) when the user closes the modal
        // while batches are still uploading.
        () => this.cancelled
      );
      if (this.cancelled) {
        // V5: the user closed the modal mid-migration. The uploads may
        // have completed on the server, but this device must not switch
        // vaults or claim success for a walk-away.
        return;
      }
      this.renderComplete(result);
    } catch (error) {
      if (!this.cancelled) this.renderFailure(error);
    } finally {
      this.running = false;
      this.updateStartState();
    }
  }

  private renderProgress(progress: MigrationProgress): void {
    if (!this.statusEl || !this.progressEl) return;
    this.statusEl.setText(this.stageLabel(progress.stage));
    this.progressEl.setText(
      format(this.labels.migrateProgressSummary, {
        processed: progress.processedFiles,
        total: progress.totalFiles,
        batch: progress.currentBatch,
        batches: progress.totalBatches,
        blobs: progress.uploadedBlobs,
        totalBlobs: progress.totalBlobs
      })
    );
  }

  private renderComplete(result: MigrationResult): void {
    this.statusEl?.setText(this.labels.migrateStageComplete);
    this.progressEl?.setText(
      format(this.labels.migrateCompleteSummary, {
        count: result.pushedFiles,
        skipped: result.skippedCount
      })
    );
  }

  private renderFailure(error: unknown): void {
    this.statusEl?.setText(this.labels.migrateFailed);
    this.progressEl?.setText(errorToMessage(error));
  }

  private updateStartState(): void {
    if (!this.startButton || !this.vaultNameInput) return;
    this.startButton.disabled =
      this.running || this.vaultNameInput.value?.trim().length === 0;
  }

  private stageLabel(stage: MigrationProgress["stage"]): string {
    switch (stage) {
      case "scanning":
        return this.labels.migrateStageScanning;
      case "creating_vault":
        return this.labels.migrateStageCreatingVault;
      case "uploading_blobs":
        return this.labels.migrateStageUploadingBlobs;
      case "pushing":
        return this.labels.migrateStagePushing;
      case "complete":
        return this.labels.migrateStageComplete;
    }
  }
}
