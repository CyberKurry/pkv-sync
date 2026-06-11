import { type App, Modal, Notice, TFile } from "obsidian";
import type { ConflictPair } from "../sync/conflict-files";
import {
  acceptLocal,
  acceptRemote,
  markMergeMarkersResolved
} from "../sync/resolve";
import {
  lineDiffSideBySide,
  type SideBySideDiffRow
} from "../sync/unified-diff";
import type { Strings } from "../i18n";
import { format } from "../i18n";

const TEXT_DETECT_EXTENSIONS = new Set([
  "md",
  "canvas",
  "base",
  "json",
  "txt",
  "css",
  "html",
  "xml",
  "yaml",
  "yml",
  "toml",
  "ini",
  "csv",
  "tsv",
  "log",
  "tex",
  "rst"
]);

function mergeMarkerLineClass(line: string): string {
  if (line.startsWith("<<<<<<<")) return "pkvsync-merge-marker-local";
  if (line === "=======") return "pkvsync-merge-marker-separator";
  if (line.startsWith(">>>>>>>")) return "pkvsync-merge-marker-remote";
  return "pkvsync-merge-marker-content";
}

function isLikelyText(path: string): boolean {
  const idx = path.lastIndexOf(".");
  if (idx === -1) return false;
  const ext = path.slice(idx + 1).toLowerCase();
  return TEXT_DETECT_EXTENSIONS.has(ext);
}

function looksBinary(sample: string): boolean {
  const limit = Math.min(sample.length, 8000);
  for (let i = 0; i < limit; i += 1) {
    const code = sample.charCodeAt(i);
    if (code === 0) return true;
    // Replacement character (U+FFFD) signals decode failure
    if (code === 0xfffd) return true;
  }
  return false;
}

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
    this.modalEl.addClass("pkvsync-modal-conflict-resolve");
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
    if (this.pair.kind === "merge_markers") {
      await this.renderMergeMarkerFlow();
      this.renderMergeMarkerActions();
      return;
    }

    const treatAsBinary =
      !isLikelyText(this.pair.originalPath) &&
      !isLikelyText(this.pair.conflictPath);

    if (treatAsBinary) {
      this.renderBinaryNotice();
      this.renderActions();
      return;
    }

    try {
      const originalFile = this.app.vault.getAbstractFileByPath(
        this.pair.originalPath
      );
      const originalContent =
        originalFile instanceof TFile
          ? await this.app.vault.read(originalFile)
          : "";
      const conflictContent = await this.app.vault.read(
        this.pair.conflictFile
      );

      if (looksBinary(originalContent) || looksBinary(conflictContent)) {
        this.renderBinaryNotice();
      } else {
        this.renderDiff(originalContent, conflictContent);
      }
    } catch {
      this.renderBinaryNotice();
    }

    this.renderActions();
  }

  private async renderMergeMarkerFlow(): Promise<void> {
    this.contentEl.createDiv({
      cls: "pkvsync-conflict-kind",
      text: this.labels.conflictKindMergeMarkers
    });

    try {
      const conflictContent = await this.app.vault.read(
        this.pair.conflictFile
      );
      if (looksBinary(conflictContent)) {
        this.renderBinaryNotice();
      } else {
        this.renderMergeMarkerPreview(conflictContent);
      }
    } catch {
      this.renderBinaryNotice();
    }
  }

  private renderMergeMarkerPreview(content: string): void {
    const container = this.contentEl.createDiv({
      cls: "pkvsync-merge-marker-preview"
    });
    const lines = content.split(/\r?\n/);
    const visibleLines = lines.slice(0, 1000);
    visibleLines.forEach((line, index) => {
      const row = container.createDiv({
        cls: `pkvsync-merge-marker-row ${mergeMarkerLineClass(line)}`
      });
      row.createDiv({
        cls: "pkvsync-merge-marker-line-no",
        text: String(index + 1)
      });
      row.createDiv({
        cls: "pkvsync-merge-marker-text",
        text: line
      });
    });

    if (lines.length > visibleLines.length) {
      container.createDiv({
        cls: "pkvsync-diff-meta",
        text: "..."
      });
    }
  }

  private renderBinaryNotice(): void {
    this.contentEl.createDiv({
      cls: "pkvsync-conflict-binary",
      text: this.labels.conflictBinaryNotice
    });
  }

  private renderDiff(original: string, conflict: string): void {
    const container = this.contentEl.createDiv({
      cls: "pkvsync-conflict-diff"
    });

    const table = container.createDiv({ cls: "pkvsync-diff-split" });
    table.setAttr("role", "table");

    const header = table.createDiv({ cls: "pkvsync-diff-split-header" });
    header.createDiv({ cls: "pkvsync-diff-line-no" });
    header.createDiv({
      cls: "pkvsync-diff-header-cell",
      text: this.labels.acceptLocalButton
    });
    header.createDiv({ cls: "pkvsync-diff-line-no" });
    header.createDiv({
      cls: "pkvsync-diff-header-cell",
      text: this.labels.acceptRemoteButton
    });

    const { rows, truncated } = lineDiffSideBySide(original, conflict);
    for (const row of rows) {
      this.renderRow(table, row);
    }

    if (truncated) {
      container.createDiv({
        cls: "pkvsync-diff-meta",
        text: "..."
      });
    }
  }

  private renderRow(parent: HTMLElement, row: SideBySideDiffRow): void {
    if (row.kind === "meta" || row.kind === "hunk") {
      parent.createDiv({
        cls: `pkvsync-diff-split-row is-${row.kind} is-full`,
        text: row.text ?? ""
      });
      return;
    }

    const item = parent.createDiv({
      cls: `pkvsync-diff-split-row is-${row.kind}`
    });
    item.createDiv({
      cls: "pkvsync-diff-line-no",
      text: row.leftLine ? String(row.leftLine) : ""
    });
    item.createDiv({
      cls: `pkvsync-diff-cell ${this.leftCellClass(row.kind)}`,
      text: row.leftText ?? ""
    });
    item.createDiv({
      cls: "pkvsync-diff-line-no",
      text: row.rightLine ? String(row.rightLine) : ""
    });
    item.createDiv({
      cls: `pkvsync-diff-cell ${this.rightCellClass(row.kind)}`,
      text: row.rightText ?? ""
    });
  }

  private leftCellClass(kind: SideBySideDiffRow["kind"]): string {
    if (kind === "del" || kind === "modify") return "pkvsync-diff-del";
    if (kind === "add") return "pkvsync-diff-empty";
    return "pkvsync-diff-context";
  }

  private rightCellClass(kind: SideBySideDiffRow["kind"]): string {
    if (kind === "add" || kind === "modify") return "pkvsync-diff-add";
    if (kind === "del") return "pkvsync-diff-empty";
    return "pkvsync-diff-context";
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

  private renderMergeMarkerActions(): void {
    const actions = this.contentEl.createDiv({
      cls: "pkvsync-conflict-actions"
    });

    const openBtn = actions.createEl("button", {
      cls: "pkvsync-button is-secondary",
      text: this.labels.openInEditor
    });
    openBtn.addEventListener("click", () => void this.handleOpenInEditor());

    const markResolvedBtn = actions.createEl("button", {
      cls: "pkvsync-button is-danger",
      text: this.labels.markResolved
    });
    markResolvedBtn.addEventListener(
      "click",
      () => void this.handleMarkResolved()
    );

    const dismissBtn = actions.createEl("button", {
      cls: "pkvsync-button is-ghost",
      text: this.labels.dismissConflictButton
    });
    dismissBtn.addEventListener("click", () => this.close());
  }

  private async handleOpenInEditor(): Promise<void> {
    const leaf = this.app.workspace.getLeaf(false);
    await leaf.openFile(this.pair.conflictFile);
  }

  private async handleAcceptLocal(): Promise<void> {
    try {
      await acceptLocal(this.app.vault, this.pair);
      new Notice(this.labels.conflictAcceptedLocalNotice);
      this.close();
      this.onResolved();
    } catch {
      new Notice(this.labels.conflictResolveFailed);
    }
  }

  private async handleAcceptRemote(): Promise<void> {
    try {
      await acceptRemote(this.app.vault, this.pair);
      new Notice(this.labels.conflictAcceptedRemoteNotice);
      this.close();
      this.onResolved();
    } catch {
      new Notice(this.labels.conflictResolveFailed);
    }
  }

  private async handleMarkResolved(): Promise<void> {
    try {
      const resolved = await markMergeMarkersResolved(this.app.vault, this.pair);
      if (!resolved) {
        new Notice(this.labels.markersStillPresent);
        return;
      }
      new Notice(this.labels.conflictAcceptedRemoteNotice);
      this.close();
      this.onResolved();
    } catch {
      new Notice(this.labels.conflictResolveFailed);
    }
  }
}
