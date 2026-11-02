import { App, Modal } from "obsidian";
import { format } from "../i18n";
import { modalButton } from "./modal-buttons";

export interface RestoreConfirmLabels {
  restoreConfirmTitle: string;
  restoreConfirmBody: string;
  restoreUnsyncedWarning: string;
  restoreCancel: string;
  restoreConfirm: string;
}

export interface RestoreConfirmParams {
  app: App;
  fileName: string;
  atCommitShort: string;
  atTimeRelative: string;
  hasUnsyncedLocalChanges: boolean;
  labels: RestoreConfirmLabels;
  onConfirm: () => Promise<void> | void;
}

export class RestoreConfirmModal extends Modal {
  constructor(private params: RestoreConfirmParams) {
    super(params.app);
  }

  onOpen(): void {
    const { labels } = this.params;
    this.contentEl.empty();
    this.contentEl.addClass("pkvsync-restore-modal");
    this.contentEl.createEl("h2", {
      text: format(labels.restoreConfirmTitle, {
        fileName: this.params.fileName,
        commit: this.params.atCommitShort
      })
    });
    this.contentEl.createDiv({
      cls: "pkvsync-restore-body",
      text: format(labels.restoreConfirmBody, {
        time: this.params.atTimeRelative,
        commit: this.params.atCommitShort
      })
    });
    if (this.params.hasUnsyncedLocalChanges) {
      this.contentEl.createDiv({
        cls: "pkvsync-restore-warning",
        text: labels.restoreUnsyncedWarning
      });
    }

    const actions = this.contentEl.createDiv({ cls: "pkvsync-restore-actions" });
    modalButton(actions, labels.restoreCancel, () => this.close());
    let confirm!: HTMLButtonElement;
    confirm = modalButton(actions, labels.restoreConfirm, async () => {
      confirm.disabled = true;
      try {
        await this.params.onConfirm();
        this.close();
      } finally {
        confirm.disabled = false;
      }
    });
    confirm.addClass("is-danger");
  }

  onClose(): void {
    this.contentEl.empty();
  }

}
