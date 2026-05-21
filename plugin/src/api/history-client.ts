import { ApiClient } from "./client";
import type {
  CommitDetail,
  CommitSummary,
  HistoricalFile,
  UnifiedDiff
} from "./types";

export class HistoryApi {
  constructor(private api: ApiClient) {}

  commits(vaultId: string, limit = 50, path?: string): Promise<CommitSummary[]> {
    const query = new URLSearchParams();
    query.set("limit", String(clampLimit(limit)));
    if (path) query.set("path", path);
    return this.api.request<CommitSummary[]>(
      "GET",
      `/api/vaults/${vaultId}/commits?${query.toString()}`,
      undefined,
      true
    );
  }

  commitDetail(vaultId: string, commit: string): Promise<CommitDetail> {
    return this.api.request<CommitDetail>(
      "GET",
      `/api/vaults/${vaultId}/commits/${encodeURIComponent(commit)}`,
      undefined,
      true
    );
  }

  fileHistory(
    vaultId: string,
    path: string,
    limit = 50
  ): Promise<CommitSummary[]> {
    return this.api.request<CommitSummary[]>(
      "GET",
      `/api/vaults/${vaultId}/history?path=${encodeURIComponent(path)}&limit=${clampLimit(limit)}`,
      undefined,
      true
    );
  }

  diff(
    vaultId: string,
    opts: { to: string; path: string; from?: string }
  ): Promise<UnifiedDiff> {
    let query = `to=${encodeURIComponent(opts.to)}&path=${encodeURIComponent(opts.path)}`;
    if (opts.from) query += `&from=${encodeURIComponent(opts.from)}`;
    return this.api.request<UnifiedDiff>(
      "GET",
      `/api/vaults/${vaultId}/diff?${query}`,
      undefined,
      true
    );
  }

  async readFileAt(
    vaultId: string,
    path: string,
    at: string
  ): Promise<HistoricalFile> {
    const encodedPath = encodePathSegments(path);
    const result = await this.api.request<string | ArrayBuffer>(
      "GET",
      `/api/vaults/${vaultId}/files/${encodedPath}?at=${encodeURIComponent(at)}`,
      undefined,
      true
    );
    return typeof result === "string"
      ? { kind: "text", text: result }
      : { kind: "binary", bytes: result };
  }
}

function clampLimit(limit: number): number {
  return Math.max(1, Math.min(200, Math.trunc(limit)));
}

export function encodePathSegments(path: string): string {
  return path.split("/").map(encodeURIComponent).join("/");
}
