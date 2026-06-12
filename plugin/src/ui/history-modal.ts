import { App, Menu, Modal, setIcon } from "obsidian";
import type { HistoryApi } from "../api/history-client";
import type { CommitSummary } from "../api/types";
import { formatUnixSeconds } from "../time";
import { errorToMessage } from "../util";

export interface HistoryEntryView {
  commit: string;
  title: string;
  device: string;
  time: string;
  message: string;
  changeType: string;
  canRestore: boolean;
  canRollback: boolean;
}

export interface HistoryModalLabels {
  historyTitle: string;
  historyEmpty: string;
  historyRetry: string;
  historyViewDiffPrevious: string;
  historyViewDiffHead: string;
  historyViewContent: string;
  historyRestoreVersion: string;
  historyRollbackToHere?: string;
  historyMoreActions: string;
  historyUnknownDevice: string;
}

export interface HistoryModalOptions {
  api: HistoryApi;
  vaultId: string;
  path: string;
  timezone: string;
  labels: HistoryModalLabels;
  onDiffPrevious?: (entry: CommitSummary) => void | Promise<void>;
  onDiffHead?: (entry: CommitSummary) => void | Promise<void>;
  onViewContent?: (entry: CommitSummary) => void | Promise<void>;
  onRestore?: (entry: CommitSummary) => void | Promise<void>;
  onRollback?: (entry: CommitSummary) => void | Promise<void>;
}

export function shortCommit(commit: string | null | undefined): string {
  return commit?.slice(0, 7) || "";
}

export function historyEntryView(
  commit: CommitSummary,
  labels: Pick<HistoryModalLabels, "historyUnknownDevice"> = {
    historyUnknownDevice: "Unknown device"
  },
  timezone = "Asia/Shanghai"
): HistoryEntryView {
  const message = commit.message.trim();
  return {
    commit: commit.commit,
    title: firstMeaningfulMessageLine(message) || shortCommit(commit.commit),
    device:
      commit.author_device?.trim() ||
      parseDeviceFromMessage(message) ||
      labels.historyUnknownDevice,
    time: formatUnixSeconds(commit.timestamp, timezone) || String(commit.timestamp),
    message,
    changeType: commit.change_type ?? "modified",
    canRestore: commit.change_type !== "deleted",
    canRollback: commit.change_type !== "deleted"
  };
}

export class HistoryModal extends Modal {
  constructor(
    app: App,
    private options: HistoryModalOptions
  ) {
    super(app);
  }

  onOpen(): void {
    this.contentEl.empty();
    this.modalEl.addClass("pkvsync-modal-history");
    this.contentEl.addClass("pkvsync-history-modal");
    this.renderShell();
    void this.load();
  }

  onClose(): void {
    this.contentEl.empty();
  }

  private renderShell(): void {
    this.renderHeader();
    this.contentEl.createDiv({ cls: "pkvsync-history-loading", text: "Loading..." });
  }

  private async load(): Promise<void> {
    try {
      const rows = await this.options.api.fileHistory(
        this.options.vaultId,
        this.options.path,
        50
      );
      this.renderRows(rows);
    } catch (error) {
      this.renderError(errorToMessage(error));
    }
  }

  private renderRows(rows: CommitSummary[]): void {
    this.contentEl.empty();
    this.contentEl.addClass("pkvsync-history-modal");
    this.renderHeader();
    if (rows.length === 0) {
      this.contentEl.createDiv({
        cls: "pkvsync-history-empty",
        text: this.options.labels.historyEmpty
      });
      return;
    }

    const list = this.contentEl.createDiv({ cls: "pkvsync-history-list" });
    for (const row of rows) {
      const view = historyEntryView(
        row,
        this.options.labels,
        this.options.timezone
      );
      const item = list.createDiv({
        cls: `pkvsync-history-row is-${view.changeType}`
      });
      const main = item.createDiv({ cls: "pkvsync-history-main" });
      const meta = main.createDiv({ cls: "pkvsync-history-meta" });
      const titleRow = meta.createDiv({ cls: "pkvsync-history-title-row" });
      titleRow.createDiv({ cls: "pkvsync-history-title", text: view.title });
      titleRow.createDiv({
        cls: `pkvsync-history-change is-${view.changeType}`,
        text: view.changeType
      });
      const details = meta.createDiv({ cls: "pkvsync-history-details" });
      details.createSpan({ cls: "pkvsync-history-device", text: view.device });
      details
        .createSpan({ cls: "pkvsync-history-time", text: view.time })
        .setAttr("title", view.time);
      details.createSpan({
        cls: "pkvsync-history-commit",
        text: shortCommit(row.commit)
      });

      const actions = item.createDiv({ cls: "pkvsync-history-actions" });
      const primaryLabel = row.parent
        ? this.options.labels.historyViewDiffPrevious
        : this.options.labels.historyViewContent;
      const primaryHandler = row.parent
        ? () => this.options.onDiffPrevious?.(row)
        : () => this.options.onViewContent?.(row);
      if (
        (row.parent && this.options.onDiffPrevious) ||
        (!row.parent && this.options.onViewContent)
      ) {
        this.button(actions, primaryLabel, primaryHandler);
      }

      const moreBtn = actions.createEl("button", {
        cls: "pkvsync-button is-ghost pkvsync-history-more"
      });
      setIcon(moreBtn, "more-horizontal");
      moreBtn.setAttr("aria-label", this.options.labels.historyMoreActions);
      moreBtn.addEventListener("click", (evt) => {
        const menu = new Menu();
        if (this.options.onDiffHead) {
          menu.addItem((mi) =>
            mi.setTitle(this.options.labels.historyViewDiffHead).onClick(() => {
              void this.options.onDiffHead?.(row);
            })
          );
        }
        if (row.parent && view.canRestore && this.options.onViewContent) {
          menu.addItem((mi) =>
            mi.setTitle(this.options.labels.historyViewContent).onClick(() => {
              void this.options.onViewContent?.(row);
            })
          );
        }
        if (view.canRestore && this.options.onRestore) {
          menu.addSeparator();
          menu.addItem((mi) =>
            mi
              .setTitle(this.options.labels.historyRestoreVersion)
              .setWarning(true)
              .onClick(() => {
                void this.options.onRestore?.(row);
              })
          );
        }
        if (view.canRollback && this.options.onRollback) {
          menu.addItem((mi) =>
            mi
              .setTitle(
                this.options.labels.historyRollbackToHere ?? "Rollback to here"
              )
              .setWarning(true)
              .onClick(() => {
                void this.options.onRollback?.(row);
              })
          );
        }
        menu.showAtMouseEvent(evt);
      });
    }
  }

  private renderError(message: string): void {
    this.contentEl.empty();
    this.renderHeader();
    this.contentEl.createDiv({ cls: "pkvsync-history-error", text: message });
    this.button(this.contentEl, this.options.labels.historyRetry, () => this.load());
  }

  private renderHeader(): void {
    const header = this.contentEl.createDiv({ cls: "pkvsync-history-header" });
    header.createEl("h2", {
      cls: "pkvsync-history-heading",
      text: this.options.labels.historyTitle
    });
    header.createDiv({
      cls: "pkvsync-history-path",
      text: this.options.path
    });
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

function firstMeaningfulMessageLine(message: string): string {
  for (const line of message.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || /^sync:/i.test(trimmed)) continue;
    return trimmed;
  }
  return "";
}

function parseDeviceFromMessage(message: string): string {
  const first = message.split(/\r?\n/, 1)[0]?.trim() ?? "";
  const match = /^sync:\s*(.+)$/i.exec(first);
  return match?.[1]?.trim() ?? "";
}
