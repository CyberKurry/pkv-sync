import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("shared plugin types", () => {
  it("derives duplicate union types from their canonical definitions", () => {
    const settings = readFileSync(resolve(__dirname, "../src/settings.ts"), "utf8");
    const i18n = readFileSync(resolve(__dirname, "../src/i18n/index.ts"), "utf8");

    expect(settings).toContain(
      'import type { UpdateSource } from "./services/update-check";'
    );
    expect(settings).toContain("export type PluginUpdateSource = UpdateSource;");
    expect(settings).not.toContain(
      'export type PluginUpdateSource = "server" | "github";'
    );

    expect(i18n).toContain('export type Lang = Exclude<PluginLanguage, "auto">;');
    expect(i18n).not.toContain(
      'export type Lang = "en" | "zh-CN" | "zh-Hant" | "ja" | "ko";'
    );
  });
});
