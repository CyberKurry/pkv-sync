import { describe, expect, it } from "vitest";
import { writePluginSettingsWithoutAuth, writePluginSettingsPatch } from "../src/plugin-data";
import { DEFAULT_SETTINGS } from "../src/settings";

describe("writePluginSettingsWithoutAuth", () => {
  it("writes settings to data.json without auth fields", () => {
    const full = { ...DEFAULT_SETTINGS, deviceId: "d", token: "t", serverUrl: "s", deploymentKey: "k", userId: "u", username: "alice" };
    const out = writePluginSettingsWithoutAuth({ syncIndexes: { a: { lastSyncedCommit: "c", files: {} } } }, full).settings as Record<string, unknown>;
    expect(out.deviceId).toBeUndefined();
    expect(out.token).toBeUndefined();
    expect(out.serverUrl).toBeUndefined();
    expect(out.deploymentKey).toBeUndefined();
    expect(out.userId).toBeUndefined();
    expect(out.username).toBe("alice");
  });
  it("preserves existing top-level keys like syncIndexes", () => {
    const out = writePluginSettingsWithoutAuth({ syncIndexes: { a: { lastSyncedCommit: "c", files: {} } } }, { ...DEFAULT_SETTINGS });
    expect((out as Record<string, unknown>).syncIndexes).toEqual({ a: { lastSyncedCommit: "c", files: {} } });
  });
});

describe("writePluginSettingsPatch", () => {
  it("strips auth keys even when a patch includes them", () => {
    const out = writePluginSettingsPatch(
      { settings: { username: "alice" } },
      { token: "leak", serverUrl: "https://leak", language: "en" }
    ).settings as Record<string, unknown>;
    expect(out.token).toBeUndefined();
    expect(out.serverUrl).toBeUndefined();
    expect(out.language).toBe("en");
    expect(out.username).toBe("alice");
  });
});
