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
    onProgress: (progress: MigrationProgress) => void
  ): Promise<MigrationResult>;
}

export class MigrateModal extends Modal {
  private vaultNameInput: HTMLInputElement | null = null;
  private startButton: HTMLButtonElement | null = null;
  private statusEl: HTMLElement | null = null;
  private progressEl: HTMLElement | null = null;
  private running = false;

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
    this.updateStartState();
    try {
      const result = await this.options.onStart(vaultName, (progress) =>
        this.renderProgress(progress)
      );
      this.renderComplete(result);
    } catch (error) {
      this.renderFailure(error);
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
