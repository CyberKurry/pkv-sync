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

export function originalPathFor(conflictPath: string): string | null {
  const m = conflictPath.match(
    /^(.+)\.conflict-\d{4}-\d{2}-\d{2}-\d{6}-[^/]+(?:\.[^/.]+)?$/
  );
  if (!m) return null;
  return m[1];
}

export interface ConflictPair {
  originalPath: string;
  conflictPath: string;
  conflictFile: TFile;
}

export function pairConflicts(
  vault: Pick<ConflictFileVault, "getFiles">
): ConflictPair[] {
  return listConflictFiles(vault)
    .map((f) => {
      const orig = originalPathFor(f.path);
      return orig
        ? { originalPath: orig, conflictPath: f.path, conflictFile: f }
        : null;
    })
    .filter((x): x is ConflictPair => x !== null);
}
