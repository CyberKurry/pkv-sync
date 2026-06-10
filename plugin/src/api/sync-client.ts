import { ApiClient } from "./client";
import { encodePathSegments } from "./history-client";
import type { RollbackResult, VaultSettings } from "./types";
import type {
  PullResponse,
  PushChange,
  PushResponse,
  StateResponse
} from "../sync/types";

export class SyncApi {
  constructor(private api: ApiClient) {}

  getVaultSettings(vaultId: string): Promise<VaultSettings> {
    return this.api.getVaultSettings(vaultId);
  }

  state(vaultId: string, headSince: string | null): Promise<StateResponse> {
    const query = headSince ? `?head_since=${encodeURIComponent(headSince)}` : "";
    return this.api.request<StateResponse>(
      "GET",
      `/api/vaults/${vaultId}/state${query}`,
      undefined,
      true
    );
  }

  uploadCheck(vaultId: string, hashes: string[]): Promise<{ missing: string[] }> {
    return this.api.request<{ missing: string[] }>(
      "POST",
      `/api/vaults/${vaultId}/upload/check`,
      { blob_hashes: hashes },
      true
    );
  }

  async uploadBlob(
    vaultId: string,
    hash: string,
    bytes: ArrayBuffer
  ): Promise<void> {
    await this.api.request<void>(
      "POST",
      `/api/vaults/${vaultId}/upload/blob`,
      bytes,
      true,
      { "content-hash": hash }
    );
  }

  push(
    vaultId: string,
    ifMatch: string | null,
    changes: PushChange[],
    deviceName: string
  ): Promise<PushResponse> {
    const headers: Record<string, string> = {
      "idempotency-key": crypto.randomUUID()
    };
    if (ifMatch) headers["if-match"] = ifMatch;
    return this.api.request<PushResponse>(
      "POST",
      `/api/vaults/${vaultId}/push`,
      { changes, device_name: deviceName },
      true,
      headers
    );
  }

  pull(vaultId: string, since: string | null): Promise<PullResponse> {
    const query = since ? `?since=${encodeURIComponent(since)}` : "";
    return this.api.request<PullResponse>(
      "GET",
      `/api/vaults/${vaultId}/pull${query}`,
      undefined,
      true
    );
  }

  downloadBlob(vaultId: string, hash: string): Promise<ArrayBuffer> {
    return this.api.request<ArrayBuffer>(
      "GET",
      `/api/vaults/${vaultId}/blobs/${hash}`,
      undefined,
      true
    );
  }

  downloadTextFile(
    vaultId: string,
    path: string,
    atCommit: string
  ): Promise<string> {
    const encodedPath = encodePathSegments(path);
    const query = `?at=${encodeURIComponent(atCommit)}`;
    return this.api.request<string>(
      "GET",
      `/api/vaults/${vaultId}/files/${encodedPath}${query}`,
      undefined,
      true
    );
  }

  restoreVault(
    vaultId: string,
    commit: string,
    confirmName: string
  ): Promise<RollbackResult> {
    return this.api.request<RollbackResult>(
      "POST",
      `/api/vaults/${vaultId}/restore`,
      { commit, confirm_vault_name: confirmName },
      true
    );
  }
}
