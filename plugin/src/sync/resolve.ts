import type { Vault } from "obsidian";
import type { ConflictPair } from "./conflict-files";

function isTFile(obj: unknown): obj is { path: string } {
  return typeof obj === "object" && obj !== null && "path" in obj;
}

export async function acceptLocal(
  vault: Vault,
  pair: ConflictPair
): Promise<void> {
  await vault.delete(pair.conflictFile);
}

export async function acceptRemote(
  vault: Vault,
  pair: ConflictPair
): Promise<void> {
  const remoteContent = await vault.read(pair.conflictFile);
  const original = vault.getAbstractFileByPath(pair.originalPath);
  if (isTFile(original)) {
    await vault.modify(original as any, remoteContent);
  } else {
    await vault.create(pair.originalPath, remoteContent);
  }
  await vault.delete(pair.conflictFile);
}
