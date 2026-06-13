import { TFile } from "obsidian";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { en } from "../src/i18n/en";
import type { Strings } from "../src/i18n";

const modalState = vi.hoisted(() => ({
  instances: [] as Array<{ onResolved: () => void }>,
  listInstances: [] as Array<{ onResolved: () => void }>,
  listOpen: vi.fn(),
  open: vi.fn()
}));

vi.mock("../src/ui/conflict-resolve-modal", () => ({
  ConflictResolveModal: class {
    constructor(
      _app: unknown,
      _pair: unknown,
      _labels: unknown,
      onResolved: () => void
    ) {
      modalState.instances.push({ onResolved });
    }

    open(): void {
      modalState.open();
    }
  }
}));

vi.mock("../src/ui/conflicts-list-modal", () => ({
  ConflictsListModal: class {
    constructor(
      _app: unknown,
      _labels: unknown,
      onResolved: () => void,
      _pairsProvider?: unknown
    ) {
      modalState.listInstances.push({ onResolved });
    }

    open(): void {
      modalState.listOpen();
    }
  }
}));

import PKVSyncPlugin from "../src/main";

type ConflictResolutionHarness = {
  app: {
    vault: {
      getFiles(): TFile[];
      read(file: TFile): Promise<string>;
    };
  };
  pushDebouncer: { trigger(): void } | null;
  text(): Strings;
  openConflictsList(pairsProvider?: () => unknown): void;
  openConflictResolutionFor(file: TFile): void;
};

function tfile(path: string): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, { path });
  return file;
}

describe("PKVSyncPlugin conflict resolution", () => {
  beforeEach(() => {
    modalState.instances.length = 0;
    modalState.listInstances.length = 0;
    modalState.listOpen.mockClear();
    modalState.open.mockClear();
  });

  it("pushes promptly after resolving a single conflict from the file menu", async () => {
    const original = tfile("note.md");
    const conflict = tfile("note.conflict-2026-05-16-143000-phone.md");
    const trigger = vi.fn();
    const plugin = Object.create(
      PKVSyncPlugin.prototype
    ) as ConflictResolutionHarness;
    plugin.pushDebouncer = { trigger };
    plugin.text = () => en;
    plugin.app = {
      vault: {
        getFiles: () => [original, conflict],
        read: vi.fn(async () => "remote content")
      }
    };

    plugin.openConflictResolutionFor(original);
    await vi.waitFor(() => expect(modalState.open).toHaveBeenCalledTimes(1));
    expect(trigger).not.toHaveBeenCalled();

    modalState.instances[0].onResolved();

    expect(trigger).toHaveBeenCalledTimes(1);
  });

  it("pushes promptly after resolving a conflict from the conflict list", () => {
    const trigger = vi.fn();
    const plugin = Object.create(
      PKVSyncPlugin.prototype
    ) as ConflictResolutionHarness;
    plugin.pushDebouncer = { trigger };
    plugin.text = () => en;
    plugin.app = {
      vault: {
        getFiles: () => [],
        read: vi.fn(async () => "remote content")
      }
    };

    plugin.openConflictsList(() => []);

    expect(modalState.listOpen).toHaveBeenCalledTimes(1);
    expect(trigger).not.toHaveBeenCalled();

    modalState.listInstances[0].onResolved();

    expect(trigger).toHaveBeenCalledTimes(1);
    expect(modalState.listOpen).toHaveBeenCalledTimes(2);
  });
});
