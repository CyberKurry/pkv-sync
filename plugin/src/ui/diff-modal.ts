import { App, Modal } from "obsidian";
import type { HistoryApi } from "../api/history-client";
import type { CommitSummary, UnifiedDiff } from "../api/types";
import {
  parseUnifiedDiffSideBySide,
  type SideBySideDiffRow
} from "../sync/unified-diff";
import { DEFAULT_TIMEZONE, formatUnixSeconds } from "../time";
import { errorToMessage } from "../util";
import { fillDiffCell } from "./diff-cells";
import { shortCommit } from "./history-modal";

export interface DiffModalLabels {
  diffTitle: string;
  diffBinary: string;
  diffTruncated: string;
  diffFrom: string;
  diffTo: string;
  diffPrevious: string;
  diffRestoreLeft: string;
  diffRestoreRight: string;
  historyRetry: string;
}

export interface DiffModalOptions {
  api: HistoryApi;
  vaultId: string;
  path: string;
  from?: string;
  to: string;
  timezone?: string;
  allowRestoreRight?: boolean;
  labels: DiffModalLabels;
  onRestore?: (commit: string, isBinary: boolean) => void | Promise<void>;
}

export function diffTitle(
  path: string,
  from: string | null | undefined,
  to: string | null | undefined
): string {
  const range =
    from || to ? ` ${shortCommit(from) || "base"}..${shortCommit(to)}` : "";
  return `${path}${range}`;
}

export function diffRestoreTargets(
  diff: Pick<UnifiedDiff, "from" | "to">,
  options: { from?: string; to?: string; allowRestoreRight?: boolean }
): { left?: string; right?: string } {
  const left = diff.from ?? options.from;
  const right = diff.to ?? options.to;
  return {
    ...(left ? { left } : {}),
    ...(options.allowRestoreRight === false || !right ? {} : { right })
  };
}

export function commitOptionLabel(
  commit: string,
  historyRows: CommitSummary[],
  timezone = DEFAULT_TIMEZONE
): string {
  const timestamp = historyRows.find((row) => row.commit === commit)?.timestamp;
  const formatted = formatUnixSeconds(timestamp, timezone);
  return formatted ? `${shortCommit(commit)} - ${formatted}` : shortCommit(commit);
}

export class DiffModal extends Modal {
  private historyRows: CommitSummary[] = [];

  constructor(
    app: App,
    private options: DiffModalOptions
  ) {
    super(app);
  }

  onOpen(): void {
    this.contentEl.empty();
    this.modalEl.addClass("pkvsync-modal-diff");
    this.contentEl.addClass("pkvsync-diff-modal");
    this.contentEl.createEl("h2", {
      text: diffTitle(this.options.path, this.options.from, this.options.to)
    });
    this.contentEl.createDiv({ cls: "pkvsync-diff-loading", text: "Loading..." });
    void this.load();
  }

  onClose(): void {
    this.contentEl.empty();
  }

  private async load(): Promise<void> {
    try {
      const [diff, rows] = await Promise.all([
        this.options.api.diff(this.options.vaultId, {
          path: this.options.path,
          from: this.options.from,
          to: this.options.to
        }),
        this.options.api
          .fileHistory(this.options.vaultId, this.options.path, 200)
          .catch(() => this.historyRows)
      ]);
      this.historyRows = rows;
      this.renderDiff(diff);
    } catch (error) {
      this.renderError(errorToMessage(error));
    }
  }

  private renderDiff(diff: UnifiedDiff): void {
    this.contentEl.empty();
    this.contentEl.addClass("pkvsync-diff-modal");
    this.contentEl.createEl("h2", {
      text: diffTitle(diff.path, diff.from ?? this.options.from, diff.to)
    });
    this.renderRangeControls(diff);
    if (diff.truncated) {
      this.contentEl.createDiv({
        cls: "pkvsync-diff-warning",
        text: this.options.labels.diffTruncated
      });
    }
    if (diff.binary) {
      this.contentEl.createDiv({
        cls: "pkvsync-diff-binary",
        text: this.options.labels.diffBinary
      });
    } else {
      this.renderSideBySideDiff(diff.patch);
    }
    this.renderRestoreActions(diff);
  }

  private renderSideBySideDiff(patch: string): void {
    const rows = parseUnifiedDiffSideBySide(patch);
    const table = this.contentEl.createDiv({ cls: "pkvsync-diff-split" });
    table.setAttr("role", "table");
    const header = table.createDiv({ cls: "pkvsync-diff-split-header" });
    header.createDiv({ cls: "pkvsync-diff-line-no" });
    header.createDiv({ cls: "pkvsync-diff-header-cell", text: this.options.labels.diffFrom });
    header.createDiv({ cls: "pkvsync-diff-line-no" });
    header.createDiv({ cls: "pkvsync-diff-header-cell", text: this.options.labels.diffTo });

    for (const row of rows) {
      this.renderSideBySideRow(table, row);
    }
  }

  private renderSideBySideRow(parent: HTMLElement, row: SideBySideDiffRow): void {
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
    const leftCell = item.createDiv({
      cls: `pkvsync-diff-cell ${leftCellClass(row.kind)}`,
    });
    fillDiffCell(leftCell, row, "left");
    item.createDiv({
      cls: "pkvsync-diff-line-no",
      text: row.rightLine ? String(row.rightLine) : ""
    });
    const rightCell = item.createDiv({
      cls: `pkvsync-diff-cell ${rightCellClass(row.kind)}`,
    });
    fillDiffCell(rightCell, row, "right");
  }

  private renderRangeControls(diff: UnifiedDiff): void {
    const currentTo = diff.to ?? this.options.to;
    const currentFrom = diff.from ?? this.options.from ?? "";
    const commits = uniqueCommits([
      currentTo,
      currentFrom,
      ...this.historyRows.map((row) => row.commit)
    ]);
    if (commits.length === 0) return;

    const controls = this.contentEl.createDiv({ cls: "pkvsync-diff-range" });
    const from = this.commitSelect(
      controls,
      this.options.labels.diffFrom,
      commits,
      currentFrom,
      true
    );
    const to = this.commitSelect(
      controls,
      this.options.labels.diffTo,
      commits,
      currentTo,
      false
    );
    const reload = () => {
      this.options.from = from.value || undefined;
      this.options.to = to.value || currentTo;
      void this.load();
    };
    from.addEventListener("change", reload);
    to.addEventListener("change", reload);
  }

  private commitSelect(
    parent: HTMLElement,
    labelText: string,
    commits: string[],
    selected: string,
    allowPrevious: boolean
  ): HTMLSelectElement {
    const label = parent.createEl("label", { cls: "pkvsync-diff-range-field" });
    label.createSpan({ text: labelText });
    const select = label.createEl("select");
    if (allowPrevious) {
      select.createEl("option", {
        value: "",
        text: this.options.labels.diffPrevious
      });
    }
    for (const commit of commits) {
      select.createEl("option", {
        value: commit,
        text: commitOptionLabel(
          commit,
          this.historyRows,
          this.options.timezone ?? DEFAULT_TIMEZONE
        )
      });
    }
    select.value = selected;
    return select;
  }

  private renderRestoreActions(diff: UnifiedDiff): void {
    if (!this.options.onRestore) return;
    const actions = this.contentEl.createDiv({ cls: "pkvsync-diff-actions" });
    const targets = diffRestoreTargets(diff, {
      from: this.options.from,
      to: this.options.to,
      allowRestoreRight: this.options.allowRestoreRight
    });
    if (targets.left) {
      this.button(actions, this.options.labels.diffRestoreLeft, () =>
        this.options.onRestore?.(targets.left!, diff.binary)
      );
    }
    if (targets.right) {
      this.button(actions, this.options.labels.diffRestoreRight, () =>
        this.options.onRestore?.(targets.right!, diff.binary)
      ).addClass("is-danger");
    }
  }

  private renderError(message: string): void {
    this.contentEl.empty();
    this.contentEl.createEl("h2", {
      text: diffTitle(this.options.path, this.options.from, this.options.to)
    });
    this.contentEl.createDiv({ cls: "pkvsync-diff-error", text: message });
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

export function uniqueCommits(commits: Array<string | null | undefined>): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const commit of commits) {
    if (!commit || seen.has(commit)) continue;
    seen.add(commit);
    out.push(commit);
  }
  return out;
}

function leftCellClass(kind: SideBySideDiffRow["kind"]): string {
  if (kind === "del" || kind === "modify") return "pkvsync-diff-del";
  if (kind === "add") return "pkvsync-diff-empty";
  return "pkvsync-diff-context";
}

function rightCellClass(kind: SideBySideDiffRow["kind"]): string {
  if (kind === "add" || kind === "modify") return "pkvsync-diff-add";
  if (kind === "del") return "pkvsync-diff-empty";
  return "pkvsync-diff-context";
}
