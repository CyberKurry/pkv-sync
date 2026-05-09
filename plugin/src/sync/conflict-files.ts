import type { TFile } from "obsidian";

export interface ConflictFileVault {
  getFiles(): TFile[];
  delete(file: TFile): Promise<void>;
}

export function isConflictPath(path: string): boolean {
  const name = path.split("/").pop() ?? path;
  return /\.conflict-\d{4}-\d{2}-\d{2}-\d{6}-[^/]+(?:\.[^/.]+)?$/.test(
    name
  );
}

export function listConflictFiles(
  vault: Pick<ConflictFileVault, "getFiles">
): TFile[] {
  return vault.getFiles().filter((file) => isConflictPath(file.path));
}

export async function deleteConflictFiles(
  vault: ConflictFileVault
): Promise<number> {
  const files = listConflictFiles(vault);
  for (const file of files) {
    await vault.delete(file);
  }
  return files.length;
}
