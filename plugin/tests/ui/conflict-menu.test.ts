import { TFile } from "obsidian";
import { describe, expect, it, vi } from "vitest";
import {
  addConflictResolveMenuItem,
  type ConflictMenuItemLike,
  type ConflictMenuLike
} from "../../src/ui/conflict-menu";

function tfile(path: string): TFile {
  const file = Object.create(TFile.prototype) as TFile;
  Object.assign(file, { path });
  return file;
}

class FakeMenuItem implements ConflictMenuItemLike {
  title = "";
  icon = "";
  click: () => void = () => {};

  setTitle(title: string): this {
    this.title = title;
    return this;
  }

  setIcon(icon: string): this {
    this.icon = icon;
    return this;
  }

  onClick(callback: () => void): this {
    this.click = callback;
    return this;
  }
}

class FakeMenu implements ConflictMenuLike {
  items: FakeMenuItem[] = [];

  addItem(callback: (item: ConflictMenuItemLike) => void): void {
    const item = new FakeMenuItem();
    callback(item);
    this.items.push(item);
  }
}

function vault(files: TFile[]) {
  return { getFiles: () => files };
}

describe("addConflictResolveMenuItem", () => {
  it("adds a resolver entry when the clicked file is the original file", () => {
    const original = tfile("note.md");
    const conflict = tfile("note.conflict-2026-05-16-143000-phone.md");
    const menu = new FakeMenu();
    const open = vi.fn();

    const added = addConflictResolveMenuItem(
      menu,
      original,
      vault([original, conflict]),
      { resolveConflictMenu: "PKV Sync: Resolve conflict" },
      open
    );

    expect(added).toBe(true);
    expect(menu.items).toHaveLength(1);
    expect(menu.items[0].title).toBe("PKV Sync: Resolve conflict");
    expect(menu.items[0].icon).toBe("git-compare");

    menu.items[0].click();

    expect(open).toHaveBeenCalledWith(original);
  });

  it("adds a resolver entry when the clicked file is the conflict file", () => {
    const original = tfile("note.md");
    const conflict = tfile("note.conflict-2026-05-16-143000-phone.md");
    const menu = new FakeMenu();

    const added = addConflictResolveMenuItem(
      menu,
      conflict,
      vault([original, conflict]),
      { resolveConflictMenu: "PKV Sync: Resolve conflict" },
      vi.fn()
    );

    expect(added).toBe(true);
    expect(menu.items).toHaveLength(1);
  });

  it("does not add an entry when the file has no conflict pair", () => {
    const menu = new FakeMenu();

    const added = addConflictResolveMenuItem(
      menu,
      tfile("plain.md"),
      vault([tfile("plain.md")]),
      { resolveConflictMenu: "PKV Sync: Resolve conflict" },
      vi.fn()
    );

    expect(added).toBe(false);
    expect(menu.items).toEqual([]);
  });
});
