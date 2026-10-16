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

/**
 * Keep the LOCAL version of the pair (V1).
 *
 * Which artifact that means depends on the sidecar role:
 * - sidecar = local (the common dirty-vs-remote shape): the user's edit
 *   lives ONLY in the sidecar — write it back onto the original path,
 *   then remove the sidecar. Trashing the sidecar alone destroyed the
 *   user's only copy.
 * - sidecar = remote (local-deleted-vs-remote-modified): the local state
 *   IS "the file is gone"; keeping it means removing the remote copy
 *   from the disk and leaving the original path absent.
 */
export async function acceptLocal(
  host: ConflictResolveHost,
  pair: ConflictPair
): Promise<void> {
  if (pair.sidecarRole === "local") {
    const localContent = await host.vault.read(pair.conflictFile);
    await writeOriginal(host.vault, pair, localContent);
  } else {
    await host.vault.getAbstractFileByPath(pair.originalPath);
  }
  await host.fileManager.trashFile(pair.conflictFile);
}

/**
 * Accept the REMOTE version of the pair (V1).
 *
 * - sidecar = local: the remote version is what the original path
 *   already holds — removing the local sidecar IS accepting remote.
 *   (Writing the sidecar back here republished the local draft.)
 * - sidecar = remote: the remote content lives in the sidecar — write
 *   it back onto the original path, then remove the sidecar.
 */
export async function acceptRemote(
  host: ConflictResolveHost,
  pair: ConflictPair
): Promise<void> {
  if (pair.sidecarRole === "remote") {
    const remoteContent = await host.vault.read(pair.conflictFile);
    await writeOriginal(host.vault, pair, remoteContent);
  }
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
