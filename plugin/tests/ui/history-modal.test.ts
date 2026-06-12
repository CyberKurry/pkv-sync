import { describe, expect, it } from "vitest";
import {
  HistoryModal,
  historyEntryView,
  shortCommit,
  type HistoryModalLabels
} from "../../src/ui/history-modal";
import type { CommitSummary } from "../../src/api/types";

function commit(overrides: Partial<CommitSummary> = {}): CommitSummary {
  return {
    commit: "1234567890abcdef",
    parent: "abcdef1234567890",
    message: "sync: Laptop\n\nUpdated note",
    timestamp: 1_700_000_000,
    author_device: "Laptop",
    change_type: "modified",
    ...overrides
  };
}

describe("history modal helpers", () => {
  it("shortens commit ids for compact rows", () => {
    expect(shortCommit("1234567890abcdef")).toBe("1234567");
  });

  it("builds readable history row data and hides restore for deleted entries", () => {
    expect(historyEntryView(commit()).canRestore).toBe(true);
    expect(historyEntryView(commit()).canRollback).toBe(true);

    const view = historyEntryView(
      commit({
        message: "",
        author_device: null,
        change_type: "deleted"
      })
    );

    expect(view.title).toBe("1234567");
    expect(view.device).toBe("Unknown device");
    expect(view.canRestore).toBe(false);
    expect(view.canRollback).toBe(false);
  });

  it("exposes rollback separately from file restore", () => {
    const view = historyEntryView(commit());

    expect(view.canRestore).toBe(true);
    expect(view.canRollback).toBe(true);
  });
});

describe("HistoryModal rows", () => {
  it("renders one primary action and an overflow menu trigger per row", () => {
    const contentEl = renderRows([
      commit(),
      commit({
        commit: "fedcba0987654321",
        parent: null,
        message: "Initial import"
      })
    ]);

    const rows = contentEl.querySelectorAll(".pkvsync-history-row");
    expect(rows.length).toBe(2);
    for (const row of rows) {
      const primaries = row
        .querySelectorAll("button.pkvsync-button")
        .filter((button) => !button.hasClass("pkvsync-history-more"));
      expect(primaries.length).toBe(1);
      expect(row.querySelector(".pkvsync-history-more")).toBeTruthy();
    }
  });

  it("uses view-content as primary for the root commit", () => {
    const contentEl = renderRows([
      commit({
        parent: null,
        message: "Initial import"
      })
    ]);

    const row = contentEl.querySelector(".pkvsync-history-row");
    const primary = row?.querySelector("button.pkvsync-button");
    expect(primary?.textContent).toBe(labels.historyViewContent);
  });
});

const labels: HistoryModalLabels = {
  historyTitle: "File history",
  historyEmpty: "No history",
  historyRetry: "Retry",
  historyViewDiffPrevious: "Diff with previous",
  historyViewDiffHead: "Diff with HEAD",
  historyViewContent: "View content",
  historyRestoreVersion: "Restore",
  historyRollbackToHere: "Rollback",
  historyMoreActions: "More actions",
  historyUnknownDevice: "Unknown device"
};

function renderRows(rows: CommitSummary[]): MockElement {
  const modal = Object.create(HistoryModal.prototype) as unknown as {
    contentEl: MockElement;
    options: unknown;
    renderRows: (rows: CommitSummary[]) => void;
  };
  modal.contentEl = new MockElement("div");
  modal.options = {
    api: { fileHistory: async () => rows },
    vaultId: "vault-1",
    path: "Notes/example.md",
    timezone: "Asia/Shanghai",
    labels,
    onDiffPrevious: () => undefined,
    onDiffHead: () => undefined,
    onViewContent: () => undefined,
    onRestore: () => undefined,
    onRollback: () => undefined
  };

  modal.renderRows(rows);
  return modal.contentEl;
}

class MockElement {
  private readonly classes = new Set<string>();
  private readonly children: MockElement[] = [];
  private text = "";
  private attrs = new Map<string, string>();
  private listeners = new Map<string, Array<(event: unknown) => void>>();

  constructor(private readonly tag: string) {}

  empty(): void {
    this.children.length = 0;
    this.text = "";
  }

  addClass(cls: string): void {
    this.addClassNames(cls);
  }

  addEventListener(type: string, listener: (event: unknown) => void): void {
    const listeners = this.listeners.get(type) ?? [];
    listeners.push(listener);
    this.listeners.set(type, listeners);
  }

  createDiv(options: { cls?: string; text?: string } = {}): MockElement {
    return this.createChild("div", options);
  }

  createSpan(options: { cls?: string; text?: string } = {}): MockElement {
    return this.createChild("span", options);
  }

  createEl(tag: string, options: { cls?: string; text?: string } = {}): MockElement {
    return this.createChild(tag, options);
  }

  setAttr(name: string, value: string): void {
    this.attrs.set(name, value);
  }

  querySelector(selector: string): MockElement | null {
    return this.querySelectorAll(selector)[0] ?? null;
  }

  querySelectorAll(selector: string): MockElement[] {
    const matches: MockElement[] = [];
    this.collect(selector, matches);
    return matches;
  }

  get textContent(): string {
    return this.text + this.children.map((child) => child.textContent).join("");
  }

  hasClass(cls: string): boolean {
    return this.classes.has(cls);
  }

  private createChild(
    tag: string,
    options: { cls?: string; text?: string }
  ): MockElement {
    const child = new MockElement(tag);
    child.addClassNames(options.cls);
    child.text = options.text ?? "";
    this.children.push(child);
    return child;
  }

  private addClassNames(cls?: string): void {
    for (const name of cls?.split(/\s+/) ?? []) {
      if (name) this.classes.add(name);
    }
  }

  private collect(selector: string, matches: MockElement[]): void {
    if (this.matches(selector)) matches.push(this);
    for (const child of this.children) {
      child.collect(selector, matches);
    }
  }

  private matches(selector: string): boolean {
    const parts = selector.split(".");
    const tag = parts[0] || undefined;
    const classes = parts.slice(tag ? 1 : 1);
    if (tag && this.tag !== tag) return false;
    if (selector.startsWith(".")) {
      return this.classes.has(selector.slice(1));
    }
    return classes.every((cls) => this.classes.has(cls));
  }
}
