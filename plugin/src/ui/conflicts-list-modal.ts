import { type App, Modal } from "obsidian";
import {
  pairConflictsWithKinds,
  type ConflictPair
} from "../sync/conflict-files";
import type { Strings } from "../i18n";
import { errorToMessage } from "../util";
import { ConflictResolveModal } from "./conflict-resolve-modal";

export class ConflictsListModal extends Modal {
  private pairs: ConflictPair[] = [];
  private isMounted = false;
  private loadGeneration = 0;

  constructor(
    app: App,
    private labels: Strings,
    private onResolved: () => void,
    private pairsProvider?: () => ConflictPair[] | Promise<ConflictPair[]>
  ) {
    super(app);
  }

  onOpen(): void {
    this.isMounted = true;
    this.contentEl.empty();
    this.contentEl.addClass("pkvsync-conflicts-list-modal");
    this.contentEl.createEl("h2", { text: this.labels.conflictsListTitle });
    void this.loadPairs();
  }

  onClose(): void {
    this.isMounted = false;
    this.loadGeneration += 1;
    this.contentEl.empty();
  }

  private canRender(generation: number): boolean {
    return this.isMounted && this.loadGeneration === generation;
  }

  private async loadPairs(): Promise<void> {
    const generation = ++this.loadGeneration;
    let pairs: ConflictPair[];
    try {
      pairs = this.pairsProvider
        ? await this.pairsProvider()
        : await pairConflictsWithKinds(this.app.vault);
    } catch (error) {
      // V7: a sidecar that vanished between listing and reading made the
      // whole Promise.all reject — the modal showed a bare title with no
      // rows, no error, and no retry.
      if (!this.canRender(generation)) return;
      this.renderError(errorToMessage(error));
      return;
    }
    if (!this.canRender(generation)) return;
    this.pairs = pairs;

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

  private renderError(message: string): void {
    this.contentEl.createDiv({
      cls: "pkvsync-conflicts-error",
      text: `${this.labels.conflictsListLoadFailed}: ${message}`
    });
    const actions = this.contentEl.createDiv({
      cls: "pkvsync-conflict-actions"
    });
    const retry = actions.createEl("button", {
      cls: "pkvsync-button is-primary",
      text: this.labels.historyRetry
    });
    retry.addEventListener("click", () => {
      this.contentEl.empty();
      this.contentEl.createEl("h2", {
        text: this.labels.conflictsListTitle
      });
      void this.loadPairs();
    });
  }
}
