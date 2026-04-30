import { type RequestUrlParam, requestUrl } from "obsidian";
import {
  type AuthResponse,
  type MeResponse,
  type ServerConfigResponse,
  type TokenView,
  type VaultSummary
} from "./types";

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string
  ) {
    super(message);
  }
}

export interface ApiClientOptions {
  serverUrl: string;
  deploymentKey: string;
  token?: string;
  pluginVersion: string;
}

export class ApiClient {
  constructor(private opts: ApiClientOptions) {}

  update(opts: Partial<ApiClientOptions>): void {
    this.opts = { ...this.opts, ...opts };
  }

  async config(): Promise<ServerConfigResponse> {
    return this.request<ServerConfigResponse>("GET", "/api/config");
  }

  async login(
    username: string,
    password: string,
    deviceName: string
  ): Promise<AuthResponse> {
    return this.request<AuthResponse>("POST", "/api/auth/login", {
      username,
      password,
      device_name: deviceName
    });
  }

  async register(
    username: string,
    password: string,
    deviceName: string,
    inviteCode?: string
  ): Promise<AuthResponse> {
    return this.request<AuthResponse>("POST", "/api/auth/register", {
      username,
      password,
      device_name: deviceName,
      invite_code: inviteCode
    });
  }

  async me(): Promise<MeResponse> {
    return this.request<MeResponse>("GET", "/api/me", undefined, true);
  }

  async tokens(): Promise<TokenView[]> {
    return this.request<TokenView[]>("GET", "/api/me/tokens", undefined, true);
  }

  async createVault(name: string): Promise<VaultSummary> {
    return this.request<VaultSummary>("POST", "/api/vaults", { name }, true);
  }

  async logout(): Promise<void> {
    await this.request<void>("POST", "/api/me/logout", undefined, true);
  }

  async request<T>(
    method: string,
    path: string,
    body?: unknown,
    auth = false,
    extraHeaders: Record<string, string> = {}
  ): Promise<T> {
    if (!this.opts.serverUrl || !this.opts.deploymentKey) {
      throw new ApiError(
        0,
        "not_configured",
        "Server URL and deployment key are required"
      );
    }
    if (auth && !this.opts.token) {
      throw new ApiError(0, "not_logged_in", "Login required");
    }

    const headers: Record<string, string> = {
      "User-Agent": `PKVSync-Plugin/${this.opts.pluginVersion}`,
      "X-PKVSync-Deployment-Key": this.opts.deploymentKey,
      ...extraHeaders
    };
    if (auth && this.opts.token) headers.Authorization = `Bearer ${this.opts.token}`;
    let requestBody: string | ArrayBuffer | undefined;
    if (body instanceof ArrayBuffer) {
      requestBody = body;
    } else if (body !== undefined) {
      headers["Content-Type"] = "application/json";
      requestBody = JSON.stringify(body);
    }

    const params: RequestUrlParam = {
      url: `${this.opts.serverUrl}${path}`,
      method,
      headers,
      body: requestBody,
      throw: false
    };
    const response = await requestUrl(params);
    if (response.status < 200 || response.status >= 300) {
      const parsed = tryParseError(response.text, response.status);
      throw new ApiError(response.status, parsed.code, parsed.message);
    }
    const contentType =
      response.headers?.["content-type"] ?? response.headers?.["Content-Type"] ?? "";
    if (contentType.includes("application/octet-stream")) {
      return response.arrayBuffer as T;
    }
    if (contentType.startsWith("text/")) return response.text as T;
    if (response.status === 204 || response.text.length === 0) return undefined as T;
    return JSON.parse(response.text) as T;
  }
}

function tryParseError(text: string, status: number): { code: string; message: string } {
  try {
    const value = JSON.parse(text) as {
      error?: { code?: string; message?: string };
    };
    return {
      code: value.error?.code ?? `http_${status}`,
      message: value.error?.message ?? `HTTP ${status}`
    };
  } catch {
    return { code: `http_${status}`, message: `HTTP ${status}` };
  }
}

export const __test = { tryParseError };
