export type RegistrationMode = "disabled" | "invite_only" | "open";

export interface ServerConfigResponse {
  server_name: string;
  version: string;
  registration: RegistrationMode;
  max_file_size: number;
  supported_text_extensions: string[];
  capabilities?: ServerCapabilities;
}

export interface ServerCapabilities {
  history?: boolean;
  diff?: boolean;
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
