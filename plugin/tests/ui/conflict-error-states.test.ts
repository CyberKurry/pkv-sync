import { describe, expect, it, vi } from "vitest";
import { TFile } from "obsidian";
import { en } from "../../src/i18n/en";
import type { ConflictPair } from "../../src/sync/conflict-files";
import { ConflictsListModal } from "../../src/ui/conflicts-list-modal";
import { ConflictResolveModal } from "../../src/ui/conflict-resolve-modal";

function tfile(path: string): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, { path });
  return file;
}

function pair(): ConflictPair {
  return {
    originalPath: "note.md",
    conflictPath: "note.conflict-2026-05-16-143000-abc.md",
    kind: "remote_copy",
    conflictFile: tfile("note.conflict-2026-05-16-143000-abc.md"),
    sidecarRole: "local"
  };
}

interface RecordedNode {
  tag: string;
  cls?: string;
  text?: string;
  children: RecordedNode[];
  listeners: Map<string, () => void>;
}

function makeElementRecorder(): {
  root: RecordedNode;
  element: Record<string, unknown>;
} {
  const root: RecordedNode = {
    tag: "div",
    children: [],
    listeners: new Map()
  };
  const wrap = (node: RecordedNode): Record<string, unknown> => ({
    empty: () => {
      node.children = [];
    },
    addClass: vi.fn(),
    createDiv: (opts: { cls?: string; text?: string } = {}) =>
      attach(node, "div", opts),
    createEl: (tag: string, opts: { cls?: string; text?: string } = {}) =>
      attach(node, tag, opts),
    setAttr: vi.fn(),
    setText: vi.fn((t: string) => {
      node.text = t;
    })
  });
  const attach = (
    parent: RecordedNode,
    tag: string,
    opts: { cls?: string; text?: string }
  ): Record<string, unknown> => {
    const node: RecordedNode = {
      tag,
      cls: opts.cls,
      text: opts.text,
      children: [],
      listeners: new Map()
    };
    parent.children.push(node);
    const el = wrap(node);
    return {
      ...el,
      addEventListener: (event: string, cb: () => void) => {
        node.listeners.set(event, cb);
      }
    };
  };
  const element = {
    ...wrap(root),
    addEventListener: (event: string, cb: () => void) => {
      root.listeners.set(event, cb);
    }
  };
  return { root, element };
}

function flatten(node: RecordedNode): RecordedNode[] {
  return [node, ...node.children.flatMap(flatten)];
}

describe("ConflictsListModal error handling (V7)", () => {
  it("renders an error with retry when pair loading rejects", async () => {
    const { root, element } = makeElementRecorder();
    const modal = new ConflictsListModal(
      { vault: {} } as never,
      en,
      vi.fn(),
      () => Promise.reject(new Error("sidecar vanished"))
    );
    (modal as unknown as { contentEl: unknown }).contentEl = element;

    modal.open();
    await vi.waitFor(() => {
      expect(flatten(root).some((n) => n.cls === "pkvsync-conflicts-error")).toBe(
        true
      );
    });

    const text = flatten(root)
      .map((n) => n.text ?? "")
      .join("\n");
    expect(text).toContain("Failed to load conflict files");
    expect(text).toContain("sidecar vanished");
    expect(text).toContain("Retry");
  });

  it("still renders pair rows when loading succeeds", async () => {
    const { root, element } = makeElementRecorder();
    const modal = new ConflictsListModal(
      { vault: {} } as never,
      en,
      vi.fn(),
      () => Promise.resolve([pair()])
    );
    (modal as unknown as { contentEl: unknown }).contentEl = element;

    modal.open();
    await vi.waitFor(() => {
      expect(
        flatten(root).some((n) => n.cls === "pkvsync-conflict-row")
      ).toBe(true);
    });
  });
});

describe("ConflictResolveModal read failure (V8)", () => {
  it("reports a read failure as an error with retry, not as binary, and renders no accept buttons", async () => {
    const { root, element } = makeElementRecorder();
    const original = tfile("note.md");
    const modal = new ConflictResolveModal(
      {
        vault: {
          getAbstractFileByPath: () => original,
          read: vi.fn().mockRejectedValue(new Error("EACCES"))
        }
      } as never,
      pair(),
      en,
      vi.fn()
    );
    (modal as unknown as { contentEl: unknown }).contentEl = element;

    modal.open();
    await vi.waitFor(() => {
      expect(
        flatten(root).some((n) => n.cls === "pkvsync-conflict-error")
      ).toBe(true);
    });

    const text = flatten(root)
      .map((n) => n.text ?? "")
      .join("\n");
    expect(text).toContain("Could not read the conflict file");
    expect(text).toContain("EACCES");
    expect(text).not.toContain("Binary file, diff preview not available");
    expect(text).not.toContain("Keep local");
    expect(text).not.toContain("Accept remote");
    expect(text).toContain("Retry");
  });
});
