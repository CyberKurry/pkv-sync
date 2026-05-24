export type RegistrationMode = "disabled" | "invite_only" | "open";

export interface ServerConfigResponse {
  server_name: string;
  version: string;
  registration: RegistrationMode;
  max_file_size: number;
  supported_text_extensions: string[];
  capabilities?: ServerCapabilities;
  push_debounce_ms?: number;
  inline_content_max_bytes?: number;
}

export interface ServerCapabilities {
  history?: boolean;
  diff?: boolean;
  sse?: boolean;
  git_smart_http?: boolean;
}

export interface AuthResponse {
  token: string;
  user_id: string;
  username: string;
  is_admin: boolean;
}

export interface VaultSummary {
  id: string;
  user_id: string;
  name: string;
  created_at: number;
  last_sync_at: number | null;
  size_bytes: number;
  file_count: number;
}

export interface VaultSettings {
  extra_sync_globs: string[];
}

export interface MeResponse {
  user_id: string;
  username: string;
  is_admin: boolean;
  vaults: VaultSummary[];
}

export interface TokenView {
  id: string;
  device_id: string;
  device_name: string;
  created_at: number;
  last_used_at: number | null;
  current: boolean;
}

export interface ApiErrorBody {
  error: {
    code: string;
    message: string;
  };
}

export type CommitChangeType = "added" | "modified" | "deleted";

export interface CommitChange {
  path: string;
  change_type: CommitChangeType;
  old_path: string | null;
  binary: boolean;
}

export interface CommitSummary {
  commit: string;
  parent: string | null;
  message: string;
  timestamp: number;
  author_device: string | null;
  change_type?: CommitChangeType;
}

export interface CommitDetail extends CommitSummary {
  changes: CommitChange[];
}

export interface UnifiedDiff {
  from: string | null;
  to: string | null;
  path: string;
  binary: boolean;
  truncated: boolean;
  patch: string;
}

export type HistoricalFile =
  | { kind: "text"; text: string }
  | { kind: "binary"; bytes: ArrayBuffer };

export type EventChange =
  | { kind: "text_inline"; path: string; content: string }
  | { kind: "text_ref"; path: string; size: number }
  | { kind: "blob"; path: string; blob_hash: string; size: number }
  | { kind: "delete"; path: string };

export interface CommitVaultEvent {
  kind?: "commit";
  commit: string;
  parent: string | null;
  source_device_id: string;
  at: number;
  changes: EventChange[];
}

export interface RollbackVaultEvent {
  kind: "rollback";
  commit: string;
  parent: string | null;
  source_device_id: string;
  at: number;
  from_commit: string;
  to_commit: string;
}

export type VaultEvent = CommitVaultEvent | RollbackVaultEvent;

export interface RollbackResult {
  from_commit: string | null;
  to_commit: string;
  rolled_back: boolean;
}
