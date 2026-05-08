import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, isLoggedIn, normalizeSettings } from "../src/settings";

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
});
