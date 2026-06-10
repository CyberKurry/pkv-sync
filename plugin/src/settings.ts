import { generateDeviceId } from "./device-id";
import type { UpdateSource } from "./services/update-check";

export type PluginLanguage = "auto" | "en" | "zh-CN" | "zh-Hant" | "ja" | "ko";
export type PluginUpdateSource = UpdateSource;
export type PluginThemeMode = "auto" | "light" | "dark";
const PLUGIN_LANGUAGES = new Set<PluginLanguage>([
  "auto",
  "en",
  "zh-CN",
  "zh-Hant",
  "ja",
  "ko"
]);
const PLUGIN_UPDATE_SOURCES = new Set<PluginUpdateSource>([
  "server",
  "github"
]);
const PLUGIN_THEME_MODES = new Set<PluginThemeMode>(["auto", "light", "dark"]);

export const MIN_DEBOUNCE_MS = 100;
export const MAX_DEBOUNCE_MS = 60_000;
export const DEFAULT_TEXT_EXTENSIONS = [
  "md",
  "canvas",
  "base",
  "json",
  "txt",
  "css"
] as const;

const SAFE_TEXT_EXTENSIONS = new Set<string>(DEFAULT_TEXT_EXTENSIONS);

export interface PKVSyncSettings {
  language: PluginLanguage;
  themeMode: PluginThemeMode;
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
  checkForUpdates: boolean;
  updateSource: PluginUpdateSource;
  lastUpdateCheckAt: number | null;
  pollIntervalSeconds: number;
  debounceMs: number;
  textExtensions: string[];
  extraExcludeGlobs: string[];
}

export const DEFAULT_SETTINGS: PKVSyncSettings = {
  language: "auto",
  themeMode: "auto",
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
  checkForUpdates: true,
  updateSource: "server",
  lastUpdateCheckAt: null,
  pollIntervalSeconds: 60,
  debounceMs: 250,
  textExtensions: [...DEFAULT_TEXT_EXTENSIONS],
  extraExcludeGlobs: []
};

export function normalizeSettings(
  raw: Partial<PKVSyncSettings> | null | undefined
): PKVSyncSettings {
  const settings = { ...DEFAULT_SETTINGS, ...(raw ?? {}) };
  if (!PLUGIN_LANGUAGES.has(settings.language)) {
    settings.language = DEFAULT_SETTINGS.language;
  }
  if (!PLUGIN_THEME_MODES.has(settings.themeMode)) {
    settings.themeMode = DEFAULT_SETTINGS.themeMode;
  }
  if (!settings.deviceId) settings.deviceId = generateDeviceId();
  if (!settings.timezone) settings.timezone = DEFAULT_SETTINGS.timezone;
  if (typeof settings.enableHistoryUi !== "boolean") {
    settings.enableHistoryUi = DEFAULT_SETTINGS.enableHistoryUi;
  }
  if (typeof settings.checkForUpdates !== "boolean") {
    settings.checkForUpdates = DEFAULT_SETTINGS.checkForUpdates;
  }
  if (!PLUGIN_UPDATE_SOURCES.has(settings.updateSource)) {
    settings.updateSource = DEFAULT_SETTINGS.updateSource;
  }
  if (
    typeof settings.lastSyncSuccessAt !== "number" ||
    !Number.isFinite(settings.lastSyncSuccessAt)
  ) {
    settings.lastSyncSuccessAt = null;
  }
  if (
    typeof settings.lastUpdateCheckAt !== "number" ||
    !Number.isFinite(settings.lastUpdateCheckAt)
  ) {
    settings.lastUpdateCheckAt = null;
  }
  settings.pollIntervalSeconds = finitePositiveNumber(
    settings.pollIntervalSeconds,
    DEFAULT_SETTINGS.pollIntervalSeconds
  );
  settings.debounceMs = normalizeDebounceMs(settings.debounceMs);
  settings.textExtensions = normalizeTextExtensions(settings.textExtensions);
  if (!Array.isArray(settings.extraExcludeGlobs)) {
    settings.extraExcludeGlobs = [];
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

function finitePositiveNumber(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) && value > 0
    ? value
    : fallback;
}

export function normalizeDebounceMs(
  value: unknown,
  fallback = DEFAULT_SETTINGS.debounceMs
): number {
  const safeFallback = clampDebounceMs(
    finitePositiveNumber(fallback, DEFAULT_SETTINGS.debounceMs)
  );
  if (typeof value !== "number" || !Number.isFinite(value)) return safeFallback;
  return clampDebounceMs(value);
}

function clampDebounceMs(value: number): number {
  return Math.min(MAX_DEBOUNCE_MS, Math.max(MIN_DEBOUNCE_MS, Math.round(value)));
}

export function normalizeTextExtensions(value: unknown): string[] {
  if (!Array.isArray(value) || value.some((ext) => typeof ext !== "string")) {
    return [...DEFAULT_SETTINGS.textExtensions];
  }
  const normalized = value
    .map((ext) => ext.trim().toLowerCase().replace(/^\./, ""))
    .filter(
      (ext, index, entries) =>
        SAFE_TEXT_EXTENSIONS.has(ext) && entries.indexOf(ext) === index
    );
  return normalized.length > 0 ? normalized : [...DEFAULT_SETTINGS.textExtensions];
}
