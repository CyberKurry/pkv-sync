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
    "v1",
    settings.serverUrl,
    settings.deploymentKey,
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

/**
 * @deprecated Persists the full settings object, auth fields included. Production
 * code must use writePluginSettingsWithoutAuth so auth never re-enters data.json
 * (auth lives in device-local storage). Retained only for raw-merge unit tests.
 */
export function writePluginSettings(
  raw: unknown,
  settings: PKVSyncSettings
): PluginData {
  return { ...(asPluginData(raw) ?? {}), settings };
}

export function writePluginSettingsWithoutAuth(
  raw: unknown,
  settings: PKVSyncSettings
): PluginData {
  const stripped = { ...settings } as Record<string, unknown>;
  for (const k of AUTH_KEYS_FOR_WRITE) delete stripped[k];
  return { ...(asPluginData(raw) ?? {}), settings: stripped as Partial<PKVSyncSettings> };
}

export function writePluginSettingsPatch(
  raw: unknown,
  patch: Partial<PKVSyncSettings>
): PluginData {
  const data = asPluginData(raw) ?? {};
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
  const data = asPluginData(raw) ?? {};
  return {
    ...data,
    syncIndexes: {
      ...(data.syncIndexes ?? {}),
      [scopeKey]: syncIndex
    }
  };
}

function asPluginData(raw: unknown): PluginData | null {
  if (!raw || typeof raw !== "object") return null;
  return raw as PluginData;
}
