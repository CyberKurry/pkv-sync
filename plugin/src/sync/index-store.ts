import type { LocalFileSnapshot, LocalIndex } from "./types";

const DANGEROUS_INDEX_KEYS = new Set(["__proto__", "constructor", "prototype"]);

function createFilesMap(): LocalIndex["files"] {
  return Object.create(null) as LocalIndex["files"];
}

function copyFiles(files: LocalIndex["files"]): LocalIndex["files"] {
  const copy = createFilesMap();
  for (const [path, file] of Object.entries(files)) {
    copy[path] = file;
  }
  return copy;
}

function normalizeFiles(raw: unknown): LocalIndex["files"] {
  const files = createFilesMap();
  if (!raw || typeof raw !== "object") return files;
  for (const [path, file] of Object.entries(
    raw as Record<string, LocalIndex["files"][string]>
  )) {
    if (DANGEROUS_INDEX_KEYS.has(path)) continue;
    files[path] = file;
  }
  return files;
}

export function normalizeIndex(raw: unknown): LocalIndex {
  if (!raw || typeof raw !== "object") {
    return { lastSyncedCommit: null, files: createFilesMap() };
  }
  const value = raw as Partial<LocalIndex>;
  return {
    lastSyncedCommit:
      typeof value.lastSyncedCommit === "string"
        ? value.lastSyncedCommit
        : null,
    files: normalizeFiles(value.files)
  };
}

function applyFilesAndDeletes(
  index: LocalIndex,
  commit: string | null,
  files: LocalFileSnapshot[],
  deletedPaths: string[],
  advance: boolean
): LocalIndex {
  const next: LocalIndex = {
    lastSyncedCommit: advance ? commit : index.lastSyncedCommit,
    files: copyFiles(index.files)
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
  for (const path of deletedPaths) delete next.files[path];
  return next;
}

export function markSynced(
  index: LocalIndex,
  commit: string | null,
  files: LocalFileSnapshot[]
): LocalIndex {
  return applyFilesAndDeletes(index, commit, files, [], true);
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
  return applyFilesAndDeletes(index, index.lastSyncedCommit, files, [], false);
}

/**
 * Apply file upserts and deletions in a single index copy. The push batch
 * path used to call markSynced/markFilesSynced then markDeleted/markFilesDeleted,
 * copying the whole files map twice per batch; this is the one-copy equivalent.
 */
export function markBatch(
  index: LocalIndex,
  commit: string | null,
  files: LocalFileSnapshot[],
  deletedPaths: string[],
  advance: boolean
): LocalIndex {
  return applyFilesAndDeletes(index, commit, files, deletedPaths, advance);
}

/**
 * Like `markDeleted` but does NOT advance `lastSyncedCommit`. Companion to
 * `markFilesSynced` for the non-clean merge outcome path.
 */
export function markFilesDeleted(
  index: LocalIndex,
  paths: string[]
): LocalIndex {
  return applyFilesAndDeletes(index, index.lastSyncedCommit, [], paths, false);
}

export function markDeleted(
  index: LocalIndex,
  commit: string | null,
  paths: string[]
): LocalIndex {
  return applyFilesAndDeletes(index, commit, [], paths, true);
}

export function pendingFiles(
  index: LocalIndex,
  current: LocalFileSnapshot[]
): LocalFileSnapshot[] {
  return current.filter(
    (file) => index.files[file.path]?.lastSyncedHash !== file.hash
  );
}
