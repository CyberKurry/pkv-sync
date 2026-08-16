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
  app: { vault: FakePluginVault };
  text(): Strings;
  deleteConflictFiles(): Promise<number>;
};

function tfile(path: string): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, { path });
  return file;
}

class FakePluginVault {
  trashed: Array<{ path: string; system: boolean }> = [];

  constructor(private paths: string[]) {}

  getFiles(): TFile[] {
    return this.paths.map((path) => tfile(path));
  }

  async trash(file: TFile, system: boolean): Promise<void> {
    this.trashed.push({ path: file.path, system });
  }
}

function harness(paths: string[]): ConflictDeleteHarness {
  const plugin = Object.create(
    PKVSyncPlugin.prototype
  ) as ConflictDeleteHarness;
  plugin.text = () => en;
  plugin.app = { vault: new FakePluginVault(paths) };
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
    expect(plugin.app.vault.trashed).toEqual([]);

    modalState.instances[0].onClose();
    await expect(pending).resolves.toBe(0);
    expect(plugin.app.vault.trashed).toEqual([]);
  });

  it("trashes conflict files with the system trash after confirming", async () => {
    const plugin = harness([
      "note.md",
      "note.conflict-2026-04-29-143022-laptop.md",
      "img.conflict-2026-04-29-120000-phone.png"
    ]);

    const pending = plugin.deleteConflictFiles();
    await vi.waitFor(() => expect(modalState.instances).toHaveLength(1));

    await modalState.instances[0].onConfirm();
    await expect(pending).resolves.toBe(2);

    expect(plugin.app.vault.trashed).toEqual([
      { path: "note.conflict-2026-04-29-143022-laptop.md", system: true },
      { path: "img.conflict-2026-04-29-120000-phone.png", system: true }
    ]);
    expect(notices.at(-1)).toBe("Deleted 2 conflict file(s)");
  });

  it("short-circuits with a notice when there are no conflict files", async () => {
    const plugin = harness(["note.md"]);

    await expect(plugin.deleteConflictFiles()).resolves.toBe(0);

    expect(modalState.instances).toHaveLength(0);
    expect(plugin.app.vault.trashed).toEqual([]);
    expect(notices.at(-1)).toBe("No conflict files");
  });
});
