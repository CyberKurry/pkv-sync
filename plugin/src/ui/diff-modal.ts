import { App, Modal } from "obsidian";
import type { HistoryApi } from "../api/history-client";
import type { UnifiedDiff } from "../api/types";
import { parseUnifiedDiff, type DiffLineKind } from "../sync/unified-diff";
import { shortCommit } from "./history-modal";

export interface DiffModalLabels {
  diffTitle: string;
  diffBinary: string;
  diffTruncated: string;
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
      const diff = await this.options.api.diff(this.options.vaultId, {
        path: this.options.path,
        from: this.options.from,
        to: this.options.to
      });
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
