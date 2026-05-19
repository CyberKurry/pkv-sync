import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("settings theme CSS", () => {
  const css = readFileSync(resolve(__dirname, "../../styles.css"), "utf8");

  it("defines Obsidian light and dark theme palettes for the plugin settings page", () => {
    expect(css).toContain("body.theme-light .pkv-sync-settings-host");
    expect(css).toContain("body.theme-dark .pkv-sync-settings-host");
    expect(css).toContain("color-scheme: light");
    expect(css).toContain("color-scheme: dark");
    expect(css).toContain("--pkv-bg-panel: #ffffff");
    expect(css).toContain("--pkv-bg-panel: #161928");
  });
});
