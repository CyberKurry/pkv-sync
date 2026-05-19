import { TFile } from "obsidian";
import type { Strings } from "../i18n";
import {
  findConflictPairsForPath,
  type ConflictFileVault
} from "../sync/conflict-files";

export interface ConflictMenuItemLike {
  setTitle(title: string): this;
  setIcon(icon: string): this;
  onClick(callback: () => void): this;
}

export interface ConflictMenuLike {
  addItem(callback: (item: ConflictMenuItemLike) => void): void;
}

export function addConflictResolveMenuItem(
  menu: ConflictMenuLike,
  file: unknown,
  vault: Pick<ConflictFileVault, "getFiles">,
  labels: Pick<Strings, "resolveConflictMenu">,
  openConflictResolver: (file: TFile) => void
): boolean {
  if (!(file instanceof TFile)) return false;
  if (findConflictPairsForPath(vault, file.path).length === 0) return false;

  menu.addItem((item) => {
    item
      .setTitle(labels.resolveConflictMenu)
      .setIcon("git-compare")
      .onClick(() => openConflictResolver(file));
  });
  return true;
}
