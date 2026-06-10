import { type App, Modal, Notice } from "obsidian";
import { format, type Strings } from "../i18n";
import { errorToMessage } from "../util";

export interface RollbackConfirmModalOptions {
  vaultName: string;
  commit: string;
  labels: Strings;
  onConfirm: (confirmName: string) => Promise<void> | void;
}

export class RollbackConfirmModal extends Modal {
  private confirmInput: HTMLInputElement | null = null;
  private confirmButton: HTMLButtonElement | null = null;
  private submitting = false;

  constructor(
    app: App,
    private options: RollbackConfirmModalOptions
  ) {
    super(app);
  }

  onOpen(): void {
    this.contentEl.empty();
    this.contentEl.addClass("pkvsync-rollback-confirm-modal");

    this.contentEl.createEl("h2", {
      text: format(this.options.labels.rollbackConfirmTitle, {
        commit: this.options.commit
      })
    });

    this.contentEl.createDiv({
      cls: "pkvsync-rollback-confirm-body",
      text: format(this.options.labels.rollbackConfirmBody, {
        name: this.options.vaultName,
        commit: this.options.commit
      })
    });
    this.contentEl.createDiv({
      cls: "pkvsync-rollback-confirm-warning",
      text: this.options.labels.rollbackConfirmWarning
    });
    this.contentEl.createDiv({
      cls: "pkvsync-rollback-confirm-prompt",
      text: format(this.options.labels.rollbackConfirmPrompt, {
        name: this.options.vaultName
      })
    });

    this.confirmInput = this.contentEl.createEl("input", {
      cls: "pkvsync-input",
      attr: { placeholder: this.options.vaultName }
    });
    this.confirmInput.addEventListener("input", () => this.updateButtonState());

    const actions = this.contentEl.createDiv({
      cls: "pkvsync-rollback-confirm-actions"
    });
    const cancelButton = actions.createEl("button", {
      cls: "pkvsync-button is-secondary",
      text: this.options.labels.restoreCancel
    });
    cancelButton.addEventListener("click", () => this.close());

    this.confirmButton = actions.createEl("button", {
      cls: "pkvsync-button is-danger",
      text: this.options.labels.rollbackConfirmButton
    });
    this.confirmButton.disabled = true;
    this.confirmButton.addEventListener("click", () => void this.handleConfirm());
  }

  onClose(): void {
    this.contentEl.empty();
  }

  private updateButtonState(): void {
    if (!this.confirmButton || !this.confirmInput) return;
    this.confirmButton.disabled =
      this.submitting || this.confirmInput.value !== this.options.vaultName;
  }

  private async handleConfirm(): Promise<void> {
    if (
      this.submitting ||
      !this.confirmInput ||
      this.confirmInput.value !== this.options.vaultName
    ) {
      return;
    }
    this.submitting = true;
    this.updateButtonState();
    try {
      await this.options.onConfirm(this.confirmInput.value);
      this.close();
    } catch (error) {
      new Notice(`${this.options.labels.rollbackFailed}: ${errorToMessage(error)}`);
      this.submitting = false;
      this.updateButtonState();
    }
  }
}
