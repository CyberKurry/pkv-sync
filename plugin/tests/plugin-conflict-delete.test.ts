import { TFile } from "obsidian";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { en } from "../src/i18n/en";
import type { Strings } from "../src/i18n";
import { notices } from "./mocks/obsidian";

const modalState = vi.hoisted(() => ({
  instances: [] as Array<{
    count: number;
    onConfirm: () => Promise<void>;
    onClose: () => void;
  }>,
  open: vi.fn()
}));

vi.mock("../src/ui/delete-conflicts-confirm", () => ({
  DeleteConflictsConfirmModal: class {
    constructor(params: {
      count: number;
      onConfirm: () => Promise<void>;
      onClose: () => void;
    }) {
      modalState.instances.push(params);
    }

    open(): void {
      modalState.open();
    }
  }
}));

import PKVSyncPlugin from "../src/main";

type ConflictDeleteHarness = {
  app: { vault: FakePluginVault; fileManager: FakePluginFileManager };
  text(): Strings;
  deleteConflictFiles(): Promise<number>;
};

function tfile(path: string): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, { path });
  return file;
}

class FakePluginFileManager {
  trashed: string[] = [];

  constructor(private vault: FakePluginVault) {}

  async trashFile(file: TFile): Promise<void> {
    this.trashed.push(file.path);
    this.vault.remove(file.path);
  }
}

class FakePluginVault {
  constructor(private paths: string[]) {}

  getFiles(): TFile[] {
    return this.paths.map((path) => tfile(path));
  }

  remove(path: string): void {
    this.paths = this.paths.filter((candidate) => candidate !== path);
  }

  // Deprecated in Obsidian; deletion must go through FileManager.
  trash(): never {
    throw new Error("Vault#trash must not be called");
  }
}

function harness(paths: string[]): ConflictDeleteHarness {
  const plugin = Object.create(
    PKVSyncPlugin.prototype
  ) as ConflictDeleteHarness;
  const vault = new FakePluginVault(paths);
  plugin.text = () => en;
  plugin.app = { vault, fileManager: new FakePluginFileManager(vault) };
  return plugin;
}

describe("PKVSyncPlugin bulk conflict deletion", () => {
  beforeEach(() => {
    modalState.instances.length = 0;
    modalState.open.mockClear();
    notices.length = 0;
  });

  it("blocks deletion until the confirm modal is confirmed", async () => {
    const plugin = harness([
      "note.md",
      "note.conflict-2026-04-29-143022-laptop.md"
    ]);

    const pending = plugin.deleteConflictFiles();
    await vi.waitFor(() => expect(modalState.instances).toHaveLength(1));

    expect(modalState.instances[0].count).toBe(1);
    expect(plugin.app.fileManager.trashed).toEqual([]);

    modalState.instances[0].onClose();
    await expect(pending).resolves.toBe(0);
    expect(plugin.app.fileManager.trashed).toEqual([]);
  });

  it("trashes conflict files through FileManager after confirming", async () => {
    const plugin = harness([
      "note.md",
      "note.conflict-2026-04-29-143022-laptop.md",
      "img.conflict-2026-04-29-120000-phone.png"
    ]);

    const pending = plugin.deleteConflictFiles();
    await vi.waitFor(() => expect(modalState.instances).toHaveLength(1));

    await modalState.instances[0].onConfirm();
    await expect(pending).resolves.toBe(2);

    expect(plugin.app.fileManager.trashed).toEqual([
      "note.conflict-2026-04-29-143022-laptop.md",
      "img.conflict-2026-04-29-120000-phone.png"
    ]);
    expect(notices.at(-1)).toBe("Deleted 2 conflict file(s)");
  });

  it("short-circuits with a notice when there are no conflict files", async () => {
    const plugin = harness(["note.md"]);

    await expect(plugin.deleteConflictFiles()).resolves.toBe(0);

    expect(modalState.instances).toHaveLength(0);
    expect(plugin.app.fileManager.trashed).toEqual([]);
    expect(notices.at(-1)).toBe("No conflict files");
  });
});
