import { App, Modal } from "obsidian";
import type { HistoryApi } from "../api/history-client";
import type { CommitSummary, UnifiedDiff } from "../api/types";
import { parseUnifiedDiff, type DiffLineKind } from "../sync/unified-diff";
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
  allowRestoreRight?: boolean;
  labels: DiffModalLabels;
  onRestore?: (commit: string, isBinary: boolean) => void | Promise<void>;
}

export function diffLineClass(kind: DiffLineKind): string {
  return `pkvsync-diff-${kind}`;
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
      this.renderError(error instanceof Error ? error.message : String(error));
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
      const body = this.contentEl.createEl("pre", { cls: "pkvsync-diff" });
      for (const line of parseUnifiedDiff(diff.patch)) {
        body.createEl("span", {
          cls: diffLineClass(line.kind),
          text: `${line.text}\n`
        });
      }
    }
    this.renderRestoreActions(diff);
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
        text: shortCommit(commit)
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

function uniqueCommits(commits: Array<string | null | undefined>): string[] {
  const out: string[] = [];
  for (const commit of commits) {
    if (!commit || out.includes(commit)) continue;
    out.push(commit);
  }
  return out;
}
