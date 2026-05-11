import { App, Modal } from "obsidian";

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
      text: labels.restoreConfirmTitle
        .replace("{fileName}", this.params.fileName)
        .replace("{commit}", this.params.atCommitShort)
    });
    this.contentEl.createDiv({
      cls: "pkvsync-restore-body",
      text: labels.restoreConfirmBody
        .replace("{time}", this.params.atTimeRelative)
        .replace("{commit}", this.params.atCommitShort)
    });
    if (this.params.hasUnsyncedLocalChanges) {
      this.contentEl.createDiv({
        cls: "pkvsync-restore-warning",
        text: labels.restoreUnsyncedWarning
      });
    }

    const actions = this.contentEl.createDiv({ cls: "pkvsync-restore-actions" });
    this.button(actions, labels.restoreCancel, () => this.close());
    let confirm!: HTMLButtonElement;
    confirm = this.button(actions, labels.restoreConfirm, async () => {
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

  private button(
    parent: HTMLElement,
    text: string,
    onClick: () => void | Promise<void>
  ): HTMLButtonElement {
    const button = parent.createEl("button", {
      cls: "pkvsync-button",
      text
    });
    button.addEventListener("click", () => void onClick());
    return button;
  }
}
