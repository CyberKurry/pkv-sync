import { type App, Modal, Notice } from "obsidian";
import type { ConflictPair } from "../sync/conflict-files";
import { acceptLocal, acceptRemote } from "../sync/resolve";
import type { Strings } from "../i18n";
import { format } from "../i18n";

export class ConflictResolveModal extends Modal {
  constructor(
    app: App,
    private pair: ConflictPair,
    private labels: Strings,
    private onResolved: () => void
  ) {
    super(app);
  }

  onOpen(): void {
    this.contentEl.empty();
    this.contentEl.addClass("pkvsync-conflict-resolve-modal");
    this.contentEl.createEl("h2", {
      text: this.labels.conflictResolveTitle
    });
    this.contentEl.createDiv({
      cls: "pkvsync-conflict-resolve-body",
      text: format(this.labels.conflictResolveBody, {
        path: this.pair.originalPath
      })
    });

    void this.loadContent();
  }

  onClose(): void {
    this.contentEl.empty();
  }

  private async loadContent(): Promise<void> {
    try {
      const originalContent = await this.app.vault.read(
        this.app.vault.getAbstractFileByPath(
          this.pair.originalPath
        ) as any
      );
      const conflictContent = await this.app.vault.read(
        this.pair.conflictFile
      );

      this.renderDiff(originalContent, conflictContent);
    } catch {
      this.contentEl.createDiv({
        cls: "pkvsync-conflict-binary",
        text: this.labels.conflictBinaryNotice
      });
    }

    this.renderActions();
  }

  private renderDiff(original: string, conflict: string): void {
    const container = this.contentEl.createDiv({
      cls: "pkvsync-conflict-diff"
    });

    const originalSection = container.createDiv({
      cls: "pkvsync-diff-split"
    });
    const header = originalSection.createDiv({
      cls: "pkvsync-diff-split-header"
    });
    header.createDiv({
      cls: "pkvsync-diff-header-cell",
      text: this.labels.acceptLocalButton
    });
    header.createDiv({
      cls: "pkvsync-diff-header-cell",
      text: this.labels.acceptRemoteButton
    });

    const maxLines = Math.max(
      original.split("\n").length,
      conflict.split("\n").length
    );
    const lines = Math.min(maxLines, 200);
    const origLines = original.split("\n").slice(0, lines);
    const confLines = conflict.split("\n").slice(0, lines);

    for (let i = 0; i < lines; i++) {
      const row = originalSection.createDiv({
        cls: "pkvsync-diff-split-row"
      });
      row.createDiv({
        cls: "pkvsync-diff-cell pkvsync-diff-context",
        text: origLines[i] ?? ""
      });
      row.createDiv({
        cls: "pkvsync-diff-cell pkvsync-diff-context",
        text: confLines[i] ?? ""
      });
    }
  }

  private renderActions(): void {
    const actions = this.contentEl.createDiv({
      cls: "pkvsync-conflict-actions"
    });

    const localBtn = actions.createEl("button", {
      cls: "pkvsync-button is-secondary",
      text: this.labels.acceptLocalButton
    });
    localBtn.addEventListener("click", () => void this.handleAcceptLocal());

    const remoteBtn = actions.createEl("button", {
      cls: "pkvsync-button is-danger",
      text: this.labels.acceptRemoteButton
    });
    remoteBtn.addEventListener("click", () => void this.handleAcceptRemote());

    const dismissBtn = actions.createEl("button", {
      cls: "pkvsync-button is-ghost",
      text: this.labels.dismissConflictButton
    });
    dismissBtn.addEventListener("click", () => this.close());
  }

  private async handleAcceptLocal(): Promise<void> {
    try {
      await acceptLocal(this.app.vault, this.pair);
      new Notice(this.labels.conflictAcceptedLocalNotice);
      this.close();
      this.onResolved();
    } catch (error) {
      new Notice(this.labels.conflictResolveFailed);
    }
  }

  private async handleAcceptRemote(): Promise<void> {
    try {
      await acceptRemote(this.app.vault, this.pair);
      new Notice(this.labels.conflictAcceptedRemoteNotice);
      this.close();
      this.onResolved();
    } catch (error) {
      new Notice(this.labels.conflictResolveFailed);
    }
  }
}
