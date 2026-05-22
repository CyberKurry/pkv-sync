import { type App, Modal } from "obsidian";
import {
  pairConflictsWithKinds,
  type ConflictPair
} from "../sync/conflict-files";
import type { Strings } from "../i18n";
import { ConflictResolveModal } from "./conflict-resolve-modal";

export class ConflictsListModal extends Modal {
  private pairs: ConflictPair[] = [];

  constructor(
    app: App,
    private labels: Strings,
    private onResolved: () => void,
    private pairsProvider?: () => ConflictPair[] | Promise<ConflictPair[]>
  ) {
    super(app);
  }

  onOpen(): void {
    this.contentEl.empty();
    this.contentEl.addClass("pkvsync-conflicts-list-modal");
    this.contentEl.createEl("h2", { text: this.labels.conflictsListTitle });
    void this.loadPairs();
  }

  private async loadPairs(): Promise<void> {
    this.pairs = this.pairsProvider
      ? await this.pairsProvider()
      : await pairConflictsWithKinds(this.app.vault);

    if (this.pairs.length === 0) {
      this.contentEl.createDiv({
        cls: "pkvsync-conflicts-empty",
        text: this.labels.conflictsListEmpty
      });
      return;
    }

    const list = this.contentEl.createDiv({ cls: "pkvsync-conflicts-list" });
    for (const pair of this.pairs) {
      const row = list.createDiv({ cls: "pkvsync-conflict-row" });
      row.createDiv({
        cls: "pkvsync-conflict-path",
        text: pair.originalPath
      });
      row.addEventListener("click", () => {
        this.close();
        new ConflictResolveModal(
          this.app,
          pair,
          this.labels,
          () => {
            this.onResolved();
          }
        ).open();
      });
    }
  }

  onClose(): void {
    this.contentEl.empty();
  }
}
