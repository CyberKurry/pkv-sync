export type PluginLanguage = "auto" | "en" | "zh-CN";

export interface PKVSyncSettings {
  language: PluginLanguage;
  timezone: string;
  serverUrl: string;
  deploymentKey: string;
  token: string;
  username: string;
  userId: string;
  selectedVaultId: string;
  selectedVaultName: string;
  deviceId: string;
  deviceName: string;
  lastSyncSuccessAt: number | null;
  pollIntervalSeconds: number;
  debounceMs: number;
}

export const DEFAULT_SETTINGS: PKVSyncSettings = {
  language: "auto",
  timezone: "Asia/Shanghai",
  serverUrl: "",
  deploymentKey: "",
  token: "",
  username: "",
  userId: "",
  selectedVaultId: "",
  selectedVaultName: "",
  deviceId: "",
  deviceName: "",
  lastSyncSuccessAt: null,
  pollIntervalSeconds: 60,
  debounceMs: 2000
};

export function normalizeSettings(
  raw: Partial<PKVSyncSettings> | null | undefined
): PKVSyncSettings {
  const settings = { ...DEFAULT_SETTINGS, ...(raw ?? {}) };
  if (!settings.deviceId) settings.deviceId = generateDeviceId();
  if (!settings.timezone) settings.timezone = DEFAULT_SETTINGS.timezone;
  if (typeof settings.lastSyncSuccessAt !== "number") {
    settings.lastSyncSuccessAt = null;
  }
  return settings;
}

export function isLoggedIn(settings: PKVSyncSettings): boolean {
  return (
    settings.serverUrl.length > 0 &&
    settings.deploymentKey.length > 0 &&
    settings.token.length > 0
  );
}

function generateDeviceId(): string {
  const random =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
  return `dev_${random}`;
}
