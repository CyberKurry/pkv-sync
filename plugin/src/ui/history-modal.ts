import { App, Modal } from "obsidian";
import type { HistoryApi } from "../api/history-client";
import type { CommitSummary } from "../api/types";
import { formatUnixSeconds } from "../time";

export interface HistoryEntryView {
  commit: string;
  title: string;
  device: string;
  time: string;
  message: string;
  changeType: string;
  canRestore: boolean;
}

export interface HistoryModalLabels {
  historyTitle: string;
  historyEmpty: string;
  historyRetry: string;
  historyViewDiffPrevious: string;
  historyRestoreVersion: string;
  historyUnknownDevice: string;
}

export interface HistoryModalOptions {
  api: HistoryApi;
  vaultId: string;
  path: string;
  timezone: string;
  labels: HistoryModalLabels;
  onDiffPrevious?: (entry: CommitSummary) => void | Promise<void>;
  onRestore?: (entry: CommitSummary) => void | Promise<void>;
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
    canRestore: commit.change_type !== "deleted"
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
    this.contentEl.addClass("pkvsync-history-modal");
    this.renderShell();
    void this.load();
  }

  onClose(): void {
    this.contentEl.empty();
  }

  private renderShell(): void {
    this.contentEl.createEl("h2", {
      text: `${this.options.labels.historyTitle}: ${this.options.path}`
    });
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
      this.renderError(error instanceof Error ? error.message : String(error));
    }
  }

  private renderRows(rows: CommitSummary[]): void {
    this.contentEl.empty();
    this.contentEl.addClass("pkvsync-history-modal");
    this.contentEl.createEl("h2", {
      text: `${this.options.labels.historyTitle}: ${this.options.path}`
    });
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
      const meta = item.createDiv({ cls: "pkvsync-history-meta" });
      meta.createDiv({ cls: "pkvsync-history-title", text: view.title });
      meta.createDiv({
        cls: "pkvsync-history-subtitle",
        text: `${view.device} - ${view.time} - ${shortCommit(row.commit)}`
      });

      const actions = item.createDiv({ cls: "pkvsync-history-actions" });
      if (row.parent && this.options.onDiffPrevious) {
        this.button(actions, this.options.labels.historyViewDiffPrevious, () =>
          this.options.onDiffPrevious?.(row)
        );
      }
      if (view.canRestore && this.options.onRestore) {
        this.button(actions, this.options.labels.historyRestoreVersion, () =>
          this.options.onRestore?.(row)
        ).addClass("is-danger");
      }
    }
  }

  private renderError(message: string): void {
    this.contentEl.empty();
    this.contentEl.createEl("h2", {
      text: `${this.options.labels.historyTitle}: ${this.options.path}`
    });
    this.contentEl.createDiv({ cls: "pkvsync-history-error", text: message });
    this.button(this.contentEl, this.options.labels.historyRetry, () => this.load());
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
