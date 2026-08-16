import { type App, Modal } from "obsidian";
import { format, type Strings } from "../i18n";

export interface DeleteConflictsConfirmParams {
  app: App;
  count: number;
  labels: Strings;
  onConfirm: () => Promise<void> | void;
  onClose: () => void;
}

export class DeleteConflictsConfirmModal extends Modal {
  constructor(private params: DeleteConflictsConfirmParams) {
    super(params.app);
  }

  onOpen(): void {
    const { labels } = this.params;
    this.contentEl.empty();
    this.contentEl.addClass("pkvsync-delete-conflicts-modal");

    this.contentEl.createEl("h2", { text: labels.deleteConflictsConfirmTitle });

    this.contentEl.createDiv({
      cls: "pkvsync-delete-conflicts-body",
      text: format(labels.deleteConflictsConfirmMessage, {
        count: this.params.count
      })
    });

    const actions = this.contentEl.createDiv({
      cls: "pkvsync-delete-conflicts-actions"
    });

    const cancelButton = actions.createEl("button", {
      cls: "pkvsync-button is-secondary",
      text: labels.deleteConflictsCancelButton
    });
    cancelButton.addEventListener("click", () => this.close());

    const confirmButton = actions.createEl("button", {
      cls: "pkvsync-button is-danger",
      text: labels.deleteConflictsConfirmButton
    });
    confirmButton.addEventListener("click", () => void this.handleConfirm());
  }

  onClose(): void {
    this.contentEl.empty();
    this.params.onClose();
  }

  private async handleConfirm(): Promise<void> {
    try {
      await this.params.onConfirm();
    } finally {
      this.close();
    }
  }
}
