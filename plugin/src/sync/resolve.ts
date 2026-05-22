import { TFile } from "obsidian";
import { hasMergeMarkers, type ConflictPair } from "./conflict-files";

export interface ConflictResolveVault {
  read(file: TFile): Promise<string>;
  delete(file: TFile): Promise<void>;
  getAbstractFileByPath(path: string): unknown;
  modify(file: TFile, content: string): Promise<void>;
  create(path: string, content: string): Promise<TFile>;
}

function isTFile(obj: unknown): obj is TFile {
  return obj instanceof TFile;
}

async function writeOriginal(
  vault: ConflictResolveVault,
  pair: ConflictPair,
  content: string
): Promise<void> {
  const original = vault.getAbstractFileByPath(pair.originalPath);
  if (isTFile(original)) {
    await vault.modify(original, content);
  } else {
    await vault.create(pair.originalPath, content);
  }
}

export async function acceptLocal(
  vault: Pick<ConflictResolveVault, "delete">,
  pair: ConflictPair
): Promise<void> {
  await vault.delete(pair.conflictFile);
}

export async function acceptRemote(
  vault: ConflictResolveVault,
  pair: ConflictPair
): Promise<void> {
  const remoteContent = await vault.read(pair.conflictFile);
  await writeOriginal(vault, pair, remoteContent);
  await vault.delete(pair.conflictFile);
}

export async function markMergeMarkersResolved(
  vault: ConflictResolveVault,
  pair: ConflictPair
): Promise<boolean> {
  const resolvedContent = await vault.read(pair.conflictFile);
  if (hasMergeMarkers(resolvedContent)) return false;
  await writeOriginal(vault, pair, resolvedContent);
  await vault.delete(pair.conflictFile);
  return true;
}
