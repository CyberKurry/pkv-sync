import { describe, expect, it } from "vitest";
import { generateDeviceId } from "../src/device-id";
import {
  DEFAULT_SETTINGS,
  historyUiAvailable,
  isLoggedIn,
  normalizeDebounceMs,
  normalizeTextExtensions,
  normalizeSettings
} from "../src/settings";

describe("settings", () => {
  it("fills defaults", () => {
    const settings = normalizeSettings({ serverUrl: "https://x" });
    expect(settings.serverUrl).toBe("https://x");
    expect(settings.language).toBe("auto");
    expect(settings.themeMode).toBe("auto");
    expect(settings.timezone).toBe("Asia/Shanghai");
    expect(settings.deviceId).toMatch(/^dev_/);
    expect(settings.lastSyncSuccessAt).toBeNull();
    expect(settings.checkForUpdates).toBe(true);
    expect(settings.updateSource).toBe("server");
    expect(settings.lastUpdateCheckAt).toBeNull();
    expect(settings.pollIntervalSeconds).toBe(60);
    expect(settings.debounceMs).toBe(250);
    expect(settings.enableHistoryUi).toBe(true);
  });

  it("generates plugin device ids with the dev prefix", () => {
    expect(generateDeviceId()).toMatch(/^dev_/);
  });

  it("falls back when persisted numeric settings are invalid", () => {
    const settings = normalizeSettings({
      pollIntervalSeconds: "abc",
      debounceMs: Number.NaN,
      lastSyncSuccessAt: Number.NaN,
      checkForUpdates: "yes",
      updateSource: "other",
      themeMode: "neon",
      lastUpdateCheckAt: Number.NaN,
      textExtensions: [123]
    } as any);

    expect(settings.pollIntervalSeconds).toBe(60);
    expect(settings.debounceMs).toBe(250);
    expect(settings.lastSyncSuccessAt).toBeNull();
    expect(settings.checkForUpdates).toBe(true);
    expect(settings.updateSource).toBe("server");
    expect(settings.themeMode).toBe("auto");
    expect(settings.lastUpdateCheckAt).toBeNull();
    expect(settings.textExtensions).toEqual(DEFAULT_SETTINGS.textExtensions);
  });

  it("clamps debounce values to a safe client range", () => {
    expect(normalizeDebounceMs(1)).toBe(100);
    expect(normalizeDebounceMs(99)).toBe(100);
    expect(normalizeDebounceMs(250)).toBe(250);
    expect(normalizeDebounceMs(120_000)).toBe(60_000);
    expect(normalizeDebounceMs(Number.NaN, 750)).toBe(750);
  });

  it("keeps text extensions within the client safe list", () => {
    expect(
      normalizeTextExtensions([" .MD ", "json", ".env", "KEY", "sh", "css"])
    ).toEqual(["md", "json", "css"]);
    expect(normalizeTextExtensions(["env", "pem"])).toEqual(
      DEFAULT_SETTINGS.textExtensions
    );
  });

  it("isLoggedIn requires url key and token", () => {
    expect(isLoggedIn(DEFAULT_SETTINGS)).toBe(false);
    expect(
      isLoggedIn({
        ...DEFAULT_SETTINGS,
        serverUrl: "https://x",
        deploymentKey: "k",
        token: "t"
      })
    ).toBe(true);
  });

  it("history UI availability requires local settings and server capability", () => {
    const loggedIn = {
      ...DEFAULT_SETTINGS,
      serverUrl: "https://x",
      deploymentKey: "k",
      token: "t",
      selectedVaultId: "v1"
    };

    expect(historyUiAvailable(loggedIn, { history: true })).toBe(true);
    expect(
      historyUiAvailable({ ...loggedIn, enableHistoryUi: false }, { history: true })
    ).toBe(false);
    expect(historyUiAvailable(loggedIn, { history: false })).toBe(false);
    expect(historyUiAvailable({ ...loggedIn, selectedVaultId: "" }, { history: true })).toBe(
      false
    );
  });
});
