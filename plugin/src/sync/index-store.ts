import type { LocalFileSnapshot, LocalIndex } from "./types";

export const EMPTY_INDEX: LocalIndex = { lastSyncedCommit: null, files: {} };

export function normalizeIndex(raw: unknown): LocalIndex {
  if (!raw || typeof raw !== "object") return structuredClone(EMPTY_INDEX);
  const value = raw as Partial<LocalIndex>;
  return {
    lastSyncedCommit:
      typeof value.lastSyncedCommit === "string"
        ? value.lastSyncedCommit
        : null,
    files:
      value.files && typeof value.files === "object" ? value.files : {}
  };
}

export function markSynced(
  index: LocalIndex,
  commit: string | null,
  files: LocalFileSnapshot[]
): LocalIndex {
  const next: LocalIndex = {
    lastSyncedCommit: commit,
    files: { ...index.files }
  };
  const now = Date.now();
  for (const file of files) {
    next.files[file.path] = {
      lastSyncedHash: file.hash,
      lastSyncedAt: now,
      kind: file.kind,
      size: file.size
    };
  }
  return next;
}

export function markDeleted(
  index: LocalIndex,
  commit: string | null,
  paths: string[]
): LocalIndex {
  const next: LocalIndex = {
    lastSyncedCommit: commit,
    files: { ...index.files }
  };
  for (const path of paths) delete next.files[path];
  return next;
}

export function pendingFiles(
  index: LocalIndex,
  current: LocalFileSnapshot[]
): LocalFileSnapshot[] {
  return current.filter(
    (file) => index.files[file.path]?.lastSyncedHash !== file.hash
  );
}

export function deletedFiles(
  index: LocalIndex,
  current: LocalFileSnapshot[]
): string[] {
  const currentPaths = new Set(current.map((file) => file.path));
  return Object.keys(index.files).filter((path) => !currentPaths.has(path));
}
