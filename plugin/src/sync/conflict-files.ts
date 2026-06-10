import type { TFile } from "obsidian";

export interface ConflictFileVault {
  getFiles(): TFile[];
  delete(file: TFile): Promise<void>;
}

interface ConflictFileReader {
  read(file: TFile): Promise<string>;
}

export type ConflictPairKind = "remote_copy" | "merge_markers";

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
    /^(.+)\.conflict-\d{4}-\d{2}-\d{2}-\d{6}-[A-Za-z0-9_-]+(\.[^/.]+)?$/
  );
  if (!m) return null;
  return `${m[1]}${m[2] ?? ""}`;
}

export interface ConflictPair {
  originalPath: string;
  conflictPath: string;
  kind: ConflictPairKind;
  conflictFile: TFile;
}

export function hasMergeMarkers(content: string): boolean {
  return (
    content.includes("<<<<<<< local") &&
    content.includes("=======") &&
    content.includes(">>>>>>> remote")
  );
}

export function pairConflicts(
  vault: Pick<ConflictFileVault, "getFiles">
): ConflictPair[] {
  return listConflictFiles(vault)
    .map<ConflictPair | null>((f) => {
      const orig = originalPathFor(f.path);
      return orig
        ? {
            originalPath: orig,
            conflictPath: f.path,
            kind: "remote_copy" as const,
            conflictFile: f
          }
        : null;
    })
    .filter((x): x is ConflictPair => x !== null);
}

export async function pairConflictsWithKinds(
  vault: Pick<ConflictFileVault, "getFiles"> & ConflictFileReader
): Promise<ConflictPair[]> {
  const pairs = pairConflicts(vault);
  return Promise.all(
    pairs.map(async (pair) => {
      const content = await vault.read(pair.conflictFile);
      return {
        ...pair,
        kind: hasMergeMarkers(content) ? "merge_markers" : "remote_copy"
      };
    })
  );
}

export async function findConflictPairsForPathWithKinds(
  vault: Pick<ConflictFileVault, "getFiles"> & ConflictFileReader,
  path: string
): Promise<ConflictPair[]> {
  const pairs = await pairConflictsWithKinds(vault);
  return pairs.filter(
    (pair) => pair.originalPath === path || pair.conflictPath === path
  );
}

export function findConflictPairsForPath(
  vault: Pick<ConflictFileVault, "getFiles">,
  path: string
): ConflictPair[] {
  return pairConflicts(vault).filter(
    (pair) => pair.originalPath === path || pair.conflictPath === path
  );
}
