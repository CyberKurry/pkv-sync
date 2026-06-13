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

const AUTH_FIELDS = ["deviceId", "token", "serverUrl", "deploymentKey", "userId"] as const;

export type MigrationResult = {
  kind: "fresh-install" | "migrated" | "already-migrated" | "write-failed-degraded";
  strippedData: Record<string, unknown> | null;
};

function extractLegacyAuth(settings: Record<string, unknown>): AuthData | null {
  const deviceId = settings.deviceId;
  if (typeof deviceId !== "string" || deviceId.length === 0) return null;
  return {
    deviceId,
    token: (settings.token as string) ?? null,
    serverUrl: (settings.serverUrl as string) ?? "",
    deploymentKey: (settings.deploymentKey as string) ?? null,
    userId: (settings.userId as string) ?? null
  };
}

function settingsHasAuthResidue(settings: Record<string, unknown>): boolean {
  return AUTH_FIELDS.some((f) => f in settings);
}

function stripAuthFields(data: Record<string, unknown>): Record<string, unknown> {
  const settings = { ...(data.settings as Record<string, unknown> | undefined) };
  for (const f of AUTH_FIELDS) delete settings[f];
  return { ...data, settings };
}

export function authFromSettings(settings: {
  deviceId: string;
  token: string;
  serverUrl: string;
  deploymentKey: string;
  userId: string;
}): AuthData {
  return {
    deviceId: settings.deviceId,
    token: settings.token || null,
    serverUrl: settings.serverUrl,
    deploymentKey: settings.deploymentKey || null,
    userId: settings.userId || null
  };
}

export function migrateAuth(
  auth: AuthStore,
  rawData: unknown
): MigrationResult {
  const data = (rawData && typeof rawData === "object" ? rawData : {}) as Record<string, unknown>;
  const settings = (data.settings && typeof data.settings === "object"
    ? data.settings
    : {}) as Record<string, unknown>;

  if (auth.load() !== null) {
    if (settingsHasAuthResidue(settings)) {
      return { kind: "already-migrated", strippedData: stripAuthFields(data) };
    }
    return { kind: "already-migrated", strippedData: null };
  }

  const legacy = extractLegacyAuth(settings);
  if (legacy === null) {
    return { kind: "fresh-install", strippedData: null };
  }

  auth.save(legacy);
  const verify = auth.load();
  if (verify === null || verify.deviceId !== legacy.deviceId || verify.token !== legacy.token) {
    return { kind: "write-failed-degraded", strippedData: null };
  }

  return { kind: "migrated", strippedData: stripAuthFields(data) };
}
