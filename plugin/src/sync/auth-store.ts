export interface AuthData {
  deviceId: string;
  token: string | null;
  serverUrl: string;
  deploymentKey: string | null;
  userId: string | null;
}

const AUTH_KEY = "pkv-sync-auth";
const SECURE_AUTH_KIND = "electron-safe-storage";
const SECURE_AUTH_VERSION = 1;

export type LocalStorageLoad = (key: string) => unknown;
export type LocalStorageSave = (key: string, data: unknown | null) => void;
export type SafeStorageLike = {
  isEncryptionAvailable(): boolean;
  encryptString(plainText: string): string;
  decryptString(ciphertext: string): string;
};

type SecureAuthEnvelope = {
  version: 1;
  kind: typeof SECURE_AUTH_KIND;
  ciphertext: string;
};

function isAuthData(value: unknown): value is AuthData {
  if (!value || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.deviceId === "string" &&
    v.deviceId.length > 0 &&
    typeof v.serverUrl === "string" &&
    isNullableString(v.token) &&
    isNullableString(v.deploymentKey) &&
    isNullableString(v.userId)
  );
}

function isSecureAuthEnvelope(value: unknown): value is SecureAuthEnvelope {
  if (!value || typeof value !== "object") return false;
  const v = value as Record<string, unknown>;
  return (
    v.version === SECURE_AUTH_VERSION &&
    v.kind === SECURE_AUTH_KIND &&
    typeof v.ciphertext === "string" &&
    v.ciphertext.length > 0
  );
}

export class AuthStore {
  constructor(
    private loadLocal: LocalStorageLoad,
    private saveLocal: LocalStorageSave,
    private safeStorage: SafeStorageLike | null = null
  ) {}

  load(): AuthData | null {
    const raw = this.loadLocal(AUTH_KEY);
    const secure = this.loadSecure(raw);
    if (secure) return secure;
    if (!isAuthData(raw)) return null;
    if (this.activeSafeStorage()) {
      this.save(raw);
    }
    return raw;
  }

  save(auth: AuthData): void {
    const safeStorage = this.activeSafeStorage();
    if (!safeStorage) {
      this.saveLocal(AUTH_KEY, auth);
      return;
    }
    this.saveLocal(AUTH_KEY, {
      version: SECURE_AUTH_VERSION,
      kind: SECURE_AUTH_KIND,
      ciphertext: safeStorage.encryptString(JSON.stringify(auth))
    });
  }

  clear(): void {
    this.saveLocal(AUTH_KEY, null);
  }

  private activeSafeStorage(): SafeStorageLike | null {
    return this.safeStorage?.isEncryptionAvailable() === true ? this.safeStorage : null;
  }

  private loadSecure(raw: unknown): AuthData | null {
    const safeStorage = this.activeSafeStorage();
    if (!isSecureAuthEnvelope(raw) || !safeStorage) return null;
    try {
      const plain = safeStorage.decryptString(raw.ciphertext);
      const parsed = JSON.parse(plain) as unknown;
      return isAuthData(parsed) ? parsed : null;
    } catch {
      return null;
    }
  }
}

export function createElectronSafeStorage(): SafeStorageLike | null {
  const requireFn = (globalThis as { window?: { require?: (id: string) => unknown } }).window
    ?.require;
  if (!requireFn) return null;
  try {
    const electron = requireFn("electron") as {
      safeStorage?: {
        isEncryptionAvailable(): boolean;
        encryptString(plainText: string): Uint8Array;
        decryptString(ciphertext: Uint8Array): string;
      };
    };
    const safeStorage = electron.safeStorage;
    const bufferCtor = (globalThis as { Buffer?: typeof Buffer }).Buffer;
    if (!safeStorage || !bufferCtor) return null;
    return {
      isEncryptionAvailable: () => safeStorage.isEncryptionAvailable(),
      encryptString: (plainText: string) =>
        bufferCtor.from(safeStorage.encryptString(plainText)).toString("base64"),
      decryptString: (ciphertext: string) =>
        safeStorage.decryptString(bufferCtor.from(ciphertext, "base64"))
    };
  } catch {
    return null;
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
    token: stringOrNull(settings.token),
    serverUrl: typeof settings.serverUrl === "string" ? settings.serverUrl : "",
    deploymentKey: stringOrNull(settings.deploymentKey),
    userId: stringOrNull(settings.userId)
  };
}

function isNullableString(value: unknown): boolean {
  return typeof value === "string" || value === null;
}

function stringOrNull(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function hasAuthResidue(data: Record<string, unknown>): boolean {
  return (
    AUTH_FIELDS.some((f) => f in data) ||
    AUTH_FIELDS.some((f) => f in nestedSettings(data))
  );
}

function nestedSettings(data: Record<string, unknown>): Record<string, unknown> {
  return data.settings && typeof data.settings === "object"
    ? (data.settings as Record<string, unknown>)
    : {};
}

function stripAuthFields(data: Record<string, unknown>): Record<string, unknown> {
  const stripped = { ...data };
  for (const f of AUTH_FIELDS) delete stripped[f];
  if (!data.settings || typeof data.settings !== "object") return stripped;
  const settings = { ...(data.settings as Record<string, unknown>) };
  for (const f of AUTH_FIELDS) delete settings[f];
  return { ...stripped, settings };
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
  const settings = nestedSettings(data);

  if (auth.load() !== null) {
    if (hasAuthResidue(data)) {
      return { kind: "already-migrated", strippedData: stripAuthFields(data) };
    }
    return { kind: "already-migrated", strippedData: null };
  }

  const legacy = extractLegacyAuth(settings) ?? extractLegacyAuth(data);
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
