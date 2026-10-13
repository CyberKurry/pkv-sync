import { TFile } from "obsidian";
import { hasMergeMarkers, type ConflictPair } from "./conflict-files";

export interface ConflictResolveVault {
  read(file: TFile): Promise<string>;
  getAbstractFileByPath(path: string): unknown;
  modify(file: TFile, content: string): Promise<void>;
  create(path: string, content: string): Promise<TFile>;
}

/** The app surface these helpers need; accepts `App` itself. */
export interface ConflictResolveHost {
  vault: ConflictResolveVault;
  fileManager: { trashFile(file: TFile): Promise<void> };
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
  host: Pick<ConflictResolveHost, "fileManager">,
  pair: ConflictPair
): Promise<void> {
  await host.fileManager.trashFile(pair.conflictFile);
}

export async function acceptRemote(
  host: ConflictResolveHost,
  pair: ConflictPair
): Promise<void> {
  const remoteContent = await host.vault.read(pair.conflictFile);
  await writeOriginal(host.vault, pair, remoteContent);
  await host.fileManager.trashFile(pair.conflictFile);
}

export async function markMergeMarkersResolved(
  host: ConflictResolveHost,
  pair: ConflictPair
): Promise<boolean> {
  const resolvedContent = await host.vault.read(pair.conflictFile);
  if (hasMergeMarkers(resolvedContent)) return false;
  await writeOriginal(host.vault, pair, resolvedContent);
  await host.fileManager.trashFile(pair.conflictFile);
  return true;
}
