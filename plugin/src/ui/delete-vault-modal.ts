import { type App, Modal, Notice } from "obsidian";
import type { VaultSummary } from "../api/types";
import { format, type Strings } from "../i18n";
import { errorToMessage } from "../util";

export class DeleteVaultModal extends Modal {
  private confirmInput: HTMLInputElement | null = null;
  private deleteButton: HTMLButtonElement | null = null;

  constructor(
    app: App,
    private vault: VaultSummary,
    private labels: Strings,
    private onConfirm: () => Promise<void>
  ) {
    super(app);
  }

  onOpen(): void {
    this.contentEl.empty();
    this.contentEl.addClass("pkvsync-delete-vault-modal");

    this.contentEl.createEl("h2", { text: this.labels.deleteVaultModalTitle });

    this.contentEl.createDiv({
      cls: "pkvsync-delete-vault-body",
      text: format(this.labels.deleteVaultModalBody, { name: this.vault.name })
    });

    this.contentEl.createDiv({
      cls: "pkvsync-delete-vault-prompt",
      text: format(this.labels.deleteVaultConfirmPrompt, { name: this.vault.name })
    });

    this.confirmInput = this.contentEl.createEl("input", {
      cls: "pkvsync-input",
      attr: { placeholder: this.vault.name }
    });
    this.confirmInput.addEventListener("input", () => this.updateButtonState());

    const actions = this.contentEl.createDiv({ cls: "pkvsync-delete-vault-actions" });

    const cancelButton = actions.createEl("button", {
      cls: "pkvsync-button is-secondary",
      text: this.labels.deleteVaultCancelButton
    });
    cancelButton.addEventListener("click", () => this.close());

    this.deleteButton = actions.createEl("button", {
      cls: "pkvsync-button is-danger",
      text: this.labels.deleteVaultConfirmButton
    });
    this.deleteButton.disabled = true;
    this.deleteButton.addEventListener("click", () => void this.handleDelete());
  }

  onClose(): void {
    this.contentEl.empty();
  }

  private updateButtonState(): void {
    if (this.deleteButton && this.confirmInput) {
      this.deleteButton.disabled = this.confirmInput.value !== this.vault.name;
    }
  }

  private async handleDelete(): Promise<void> {
    if (this.deleteButton) this.deleteButton.disabled = true;
    try {
      await this.onConfirm();
      this.close();
    } catch (error) {
      new Notice(`${this.labels.deleteVaultFailed}: ${errorToMessage(error)}`);
      if (this.deleteButton) this.deleteButton.disabled = false;
    }
  }
}
