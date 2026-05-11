import { describe, expect, it } from "vitest";
import {
  DEFAULT_SETTINGS,
  historyUiAvailable,
  isLoggedIn,
  normalizeSettings
} from "../src/settings";

describe("settings", () => {
  it("fills defaults", () => {
    const settings = normalizeSettings({ serverUrl: "https://x" });
    expect(settings.serverUrl).toBe("https://x");
    expect(settings.language).toBe("auto");
    expect(settings.timezone).toBe("Asia/Shanghai");
    expect(settings.deviceId).toMatch(/^dev_/);
    expect(settings.lastSyncSuccessAt).toBeNull();
    expect(settings.pollIntervalSeconds).toBe(60);
    expect(settings.debounceMs).toBe(2000);
    expect(settings.enableHistoryUi).toBe(true);
  });

  it("falls back when persisted numeric settings are invalid", () => {
    const settings = normalizeSettings({
      pollIntervalSeconds: "abc",
      debounceMs: Number.NaN,
      lastSyncSuccessAt: Number.NaN,
      textExtensions: [123]
    } as any);

    expect(settings.pollIntervalSeconds).toBe(60);
    expect(settings.debounceMs).toBe(2000);
    expect(settings.lastSyncSuccessAt).toBeNull();
    expect(settings.textExtensions).toEqual(DEFAULT_SETTINGS.textExtensions);
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
