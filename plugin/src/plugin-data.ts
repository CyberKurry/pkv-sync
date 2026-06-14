import { normalizeSettings, type PKVSyncSettings } from "./settings";
import { normalizeIndex } from "./sync/index-store";
import type { LocalIndex } from "./sync/types";

export interface PluginData {
  settings?: Partial<PKVSyncSettings>;
  syncIndex?: LocalIndex;
  syncIndexes?: Record<string, LocalIndex>;
  [key: string]: unknown;
}

export function readPluginSettings(raw: unknown): PKVSyncSettings {
  const data = asPluginData(raw);
  return normalizeSettings(data?.settings ?? (data as Partial<PKVSyncSettings>));
}

export function syncScopeKey(settings: PKVSyncSettings): string {
  return [
    "v2",
    settings.serverUrl,
    settings.userId || settings.username,
    settings.selectedVaultId
  ]
    .map((part) => encodeURIComponent(part))
    .join("|");
}

export function readSyncIndex(raw: unknown, scopeKey: string): LocalIndex {
  return normalizeIndex(asPluginData(raw)?.syncIndexes?.[scopeKey]);
}

const AUTH_KEYS_FOR_WRITE = ["deviceId", "token", "serverUrl", "deploymentKey", "userId"] as const;

export function writePluginSettingsWithoutAuth(
  raw: unknown,
  settings: PKVSyncSettings
): PluginData {
  const stripped = { ...settings } as Record<string, unknown>;
  for (const k of AUTH_KEYS_FOR_WRITE) delete stripped[k];
  return {
    ...stripAuthFromPluginData(raw),
    settings: stripped as Partial<PKVSyncSettings>
  };
}

export function writePluginSettingsPatch(
  raw: unknown,
  patch: Partial<PKVSyncSettings>
): PluginData {
  const data = stripAuthFromPluginData(raw);
  const settings =
    data.settings && typeof data.settings === "object"
      ? { ...data.settings, ...patch }
      : { ...readPluginSettings(raw), ...patch };
  for (const k of AUTH_KEYS_FOR_WRITE) delete (settings as Record<string, unknown>)[k];
  return { ...data, settings };
}

export function writeSyncIndex(
  raw: unknown,
  scopeKey: string,
  syncIndex: LocalIndex
): PluginData {
  const data = stripAuthFromPluginData(raw);
  return {
    ...data,
    syncIndexes: {
      ...sanitizeSyncIndexes(data.syncIndexes),
      [scopeKey]: syncIndex
    }
  };
}

function asPluginData(raw: unknown): PluginData | null {
  if (!raw || typeof raw !== "object") return null;
  return raw as PluginData;
}

function stripAuthFromPluginData(raw: unknown): PluginData {
  const data = { ...(asPluginData(raw) ?? {}) } as PluginData;
  for (const k of AUTH_KEYS_FOR_WRITE) delete data[k];
  if (data.settings && typeof data.settings === "object") {
    const settings = { ...data.settings } as Record<string, unknown>;
    for (const k of AUTH_KEYS_FOR_WRITE) delete settings[k];
    data.settings = settings as Partial<PKVSyncSettings>;
  }
  if (data.syncIndexes) {
    data.syncIndexes = sanitizeSyncIndexes(data.syncIndexes);
  }
  return data;
}

function sanitizeSyncIndexes(
  syncIndexes: Record<string, LocalIndex> | undefined
): Record<string, LocalIndex> {
  if (!syncIndexes) return {};
  return Object.fromEntries(
    Object.entries(syncIndexes).filter(([key]) => !isLegacySecretScopeKey(key))
  );
}

function isLegacySecretScopeKey(key: string): boolean {
  return key.startsWith("v1|");
}
