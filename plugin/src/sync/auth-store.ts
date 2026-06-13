export interface AuthData {
  deviceId: string;
  token: string | null;
  serverUrl: string;
  deploymentKey: string | null;
  userId: string | null;
}

const AUTH_KEY = "pkv-sync-auth";

export type LocalStorageLoad = (key: string) => unknown;
export type LocalStorageSave = (key: string, data: unknown | null) => void;

function isAuthData(value: unknown): value is AuthData {
  if (!value || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  return typeof v.deviceId === "string" && v.deviceId.length > 0;
}

export class AuthStore {
  constructor(
    private loadLocal: LocalStorageLoad,
    private saveLocal: LocalStorageSave
  ) {}

  load(): AuthData | null {
    const raw = this.loadLocal(AUTH_KEY);
    return isAuthData(raw) ? raw : null;
  }

  save(auth: AuthData): void {
    this.saveLocal(AUTH_KEY, auth);
  }

  clear(): void {
    this.saveLocal(AUTH_KEY, null);
  }
}
