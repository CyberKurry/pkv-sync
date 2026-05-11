export type PluginLanguage = "auto" | "en" | "zh-CN";

export interface PKVSyncSettings {
  language: PluginLanguage;
  timezone: string;
  enableHistoryUi: boolean;
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
  textExtensions: string[];
}

export const DEFAULT_SETTINGS: PKVSyncSettings = {
  language: "auto",
  timezone: "Asia/Shanghai",
  enableHistoryUi: true,
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
  debounceMs: 2000,
  textExtensions: ["md", "canvas", "base", "json", "txt", "css"]
};

export function normalizeSettings(
  raw: Partial<PKVSyncSettings> | null | undefined
): PKVSyncSettings {
  const settings = { ...DEFAULT_SETTINGS, ...(raw ?? {}) };
  if (!settings.deviceId) settings.deviceId = generateDeviceId();
  if (!settings.timezone) settings.timezone = DEFAULT_SETTINGS.timezone;
  if (typeof settings.enableHistoryUi !== "boolean") {
    settings.enableHistoryUi = DEFAULT_SETTINGS.enableHistoryUi;
  }
  if (
    typeof settings.lastSyncSuccessAt !== "number" ||
    !Number.isFinite(settings.lastSyncSuccessAt)
  ) {
    settings.lastSyncSuccessAt = null;
  }
  settings.pollIntervalSeconds = finitePositiveNumber(
    settings.pollIntervalSeconds,
    DEFAULT_SETTINGS.pollIntervalSeconds
  );
  settings.debounceMs = finitePositiveNumber(
    settings.debounceMs,
    DEFAULT_SETTINGS.debounceMs
  );
  if (
    !Array.isArray(settings.textExtensions) ||
    settings.textExtensions.some((ext) => typeof ext !== "string")
  ) {
    settings.textExtensions = [...DEFAULT_SETTINGS.textExtensions];
  } else {
    settings.textExtensions = settings.textExtensions
      .map((ext) => ext.trim().toLowerCase().replace(/^\./, ""))
      .filter((ext) => ext.length > 0);
    if (settings.textExtensions.length === 0) {
      settings.textExtensions = [...DEFAULT_SETTINGS.textExtensions];
    }
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

export function historyUiAvailable(
  settings: PKVSyncSettings,
  capabilities: { history?: boolean } | null | undefined
): boolean {
  return (
    settings.enableHistoryUi &&
    isLoggedIn(settings) &&
    settings.selectedVaultId.length > 0 &&
    (capabilities?.history ?? true)
  );
}

function generateDeviceId(): string {
  const random =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
  return `dev_${random}`;
}

function finitePositiveNumber(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) && value > 0
    ? value
    : fallback;
}
