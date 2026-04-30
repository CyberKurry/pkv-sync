export type PluginLanguage = "auto" | "en" | "zh-CN";

export interface PKVSyncSettings {
  language: PluginLanguage;
  serverUrl: string;
  deploymentKey: string;
  token: string;
  username: string;
  userId: string;
  selectedVaultId: string;
  selectedVaultName: string;
  deviceName: string;
  pollIntervalSeconds: number;
  debounceMs: number;
}

export const DEFAULT_SETTINGS: PKVSyncSettings = {
  language: "auto",
  serverUrl: "",
  deploymentKey: "",
  token: "",
  username: "",
  userId: "",
  selectedVaultId: "",
  selectedVaultName: "",
  deviceName: "",
  pollIntervalSeconds: 60,
  debounceMs: 2000
};

export function normalizeSettings(
  raw: Partial<PKVSyncSettings> | null | undefined
): PKVSyncSettings {
  return { ...DEFAULT_SETTINGS, ...(raw ?? {}) };
}

export function isLoggedIn(settings: PKVSyncSettings): boolean {
  return (
    settings.serverUrl.length > 0 &&
    settings.deploymentKey.length > 0 &&
    settings.token.length > 0
  );
}
