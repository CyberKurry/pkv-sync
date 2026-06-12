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
      lastSyncedMtime: file.mtime,
      kind: file.kind,
      size: file.size
    };
  }
  return next;
}

/**
 * Like `markSynced` but does NOT advance `lastSyncedCommit`. Used after a
 * push that produced non-clean merge outcomes — per-file hashes are recorded
 * but the head stays at the old commit so the subsequent pull includes the
 * merge commit in its range.
 */
export function markFilesSynced(
  index: LocalIndex,
  files: LocalFileSnapshot[]
): LocalIndex {
  const next: LocalIndex = {
    lastSyncedCommit: index.lastSyncedCommit,
    files: { ...index.files }
  };
  const now = Date.now();
  for (const file of files) {
    next.files[file.path] = {
      lastSyncedHash: file.hash,
      lastSyncedAt: now,
      lastSyncedMtime: file.mtime,
      kind: file.kind,
      size: file.size
    };
  }
  return next;
}

/**
 * Like `markDeleted` but does NOT advance `lastSyncedCommit`. Companion to
 * `markFilesSynced` for the non-clean merge outcome path.
 */
export function markFilesDeleted(
  index: LocalIndex,
  paths: string[]
): LocalIndex {
  const next: LocalIndex = {
    lastSyncedCommit: index.lastSyncedCommit,
    files: { ...index.files }
  };
  for (const path of paths) delete next.files[path];
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
