export type FileKind = "text" | "blob";

export interface IndexedFile {
  lastSyncedHash: string;
  lastSyncedAt: number;
  kind: FileKind;
  size: number;
}

export interface LocalIndex {
  lastSyncedCommit: string | null;
  files: Record<string, IndexedFile>;
}

export interface LocalFileSnapshot {
  path: string;
  hash: string;
  size: number;
  kind: FileKind;
  content?: string;
  bytes?: ArrayBuffer;
}

export type PushChange =
  | { kind: "text"; path: string; content: string }
  | { kind: "blob"; path: string; blob_hash: string; size: number; mime?: string }
  | { kind: "delete"; path: string };

export interface PushResponse {
  new_commit: string;
  files_changed: number;
}

export interface PullFile {
  path: string;
  file_type: "text" | "blob";
  size: number;
  content_inline?: string | null;
  blob_hash?: string | null;
}

export interface PullResponse {
  from: string | null;
  to: string | null;
  added: PullFile[];
  modified: PullFile[];
  deleted: string[];
}

export interface StateResponse {
  current_head: string | null;
  changed_since: boolean;
}
