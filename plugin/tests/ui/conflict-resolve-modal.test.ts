import { TFile } from "obsidian";
import { describe, expect, it, vi } from "vitest";
import { en } from "../../src/i18n/en";
import type { ConflictPair } from "../../src/sync/conflict-files";
import { ConflictResolveModal } from "../../src/ui/conflict-resolve-modal";

function tfile(path: string): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, { path });
  return file;
}

describe("ConflictResolveModal async rendering", () => {
  it("does not render completed conflict content after the modal is closed", async () => {
    const original = tfile("note.md");
    const conflict = tfile("note.conflict-2026-05-16-143000-phone.md");
    const pendingConflict = deferred<string>();
    const pair: ConflictPair = {
      originalPath: original.path,
      conflictPath: conflict.path,
      kind: "remote_copy",
      conflictFile: conflict
    };
    const modal = new ConflictResolveModal(
      {
        vault: {
          getAbstractFileByPath: vi.fn(() => original),
          read: vi.fn((file: TFile) =>
            file === conflict ? pendingConflict.promise : Promise.resolve("local")
          )
        }
      } as never,
      pair,
      en,
      vi.fn()
    );
    const contentEl = modal.contentEl as unknown as {
      empty: ReturnType<typeof vi.fn>;
      createDiv: ReturnType<typeof vi.fn>;
    };

    modal.open();
    modal.close();
    contentEl.empty.mockClear();
    contentEl.createDiv.mockClear();

    pendingConflict.resolve("\u0000binary");
    await flushPromises();

    expect(contentEl.empty).not.toHaveBeenCalled();
    expect(contentEl.createDiv).not.toHaveBeenCalled();
  });
});

function deferred<T>(): {
  promise: Promise<T>;
  resolve(value: T): void;
  reject(error: unknown): void;
} {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function flushPromises(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}
