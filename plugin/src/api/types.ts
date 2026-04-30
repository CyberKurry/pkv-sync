export type RegistrationMode = "disabled" | "invite_only" | "open";

export interface ServerConfigResponse {
  server_name: string;
  version: string;
  registration: RegistrationMode;
  max_file_size: number;
  supported_text_extensions: string[];
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
