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

  it("defines compact aligned controls for dense settings actions", () => {
    expect(css).toContain("--pkv-control-height: 40px");
    expect(css).toContain("--pkv-compact-control-height: 36px");
    expect(css).toContain(".pkv-sync-textarea");
    expect(css).toContain("min-height: 76px");
    expect(css).toContain(".pkv-sync-allowlist-actions");
    expect(css).toContain("grid-template-columns: minmax(0, 1fr) minmax(112px, max-content)");
    expect(css).toContain(".pkv-sync-vault-actions .pkv-sync-button");
    expect(css).toContain("height: var(--pkv-compact-control-height)");
  });
});
