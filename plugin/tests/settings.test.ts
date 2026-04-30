import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, isLoggedIn, normalizeSettings } from "../src/settings";

describe("settings", () => {
  it("fills defaults", () => {
    const settings = normalizeSettings({ serverUrl: "https://x" });
    expect(settings.serverUrl).toBe("https://x");
    expect(settings.language).toBe("auto");
    expect(settings.pollIntervalSeconds).toBe(60);
    expect(settings.debounceMs).toBe(2000);
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
