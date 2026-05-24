import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("settings theme CSS", () => {
  const css = readFileSync(resolve(__dirname, "../../styles.css"), "utf8");

  it("defines Obsidian light and dark theme palettes for the plugin settings page", () => {
    expect(css).toContain("body.theme-light .pkv-sync-settings-host");
    expect(css).toContain("body.theme-dark .pkv-sync-settings-host");
    expect(css).toContain(".pkv-sync-settings-host.is-light-override");
    expect(css).toContain(".pkv-sync-settings-host.is-dark-override");
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
    expect(css).toContain("justify-content: flex-end");
    expect(css).toContain(".pkv-sync-vault-actions .pkv-sync-button");
    expect(css).toContain("height: var(--pkv-compact-control-height)");
  });

  it("keeps the language selector wide enough for localized labels", () => {
    expect(css).toContain(".pkv-sync-language-select");
    expect(css).toContain("min-width: 180px");
    expect(css).not.toContain(".pkv-sync-select-wrap.is-compact");
    expect(css).not.toContain("width: 58px");
  });

  it("renders theme mode as a single visible cycle button", () => {
    expect(css).toContain(".pkv-sync-theme-button");
    expect(css).toContain(".pkv-sync-theme-icon");
    expect(css).toContain(".pkv-sync-theme-label");
    expect(css).toContain(".pkv-sync-theme-button.is-dark");
    expect(css).toContain("[data-theme-mode=\"dark\"]");
    expect(css).not.toContain(".pkv-sync-theme-select");
  });

  it("renders secondary and ghost actions as visible buttons", () => {
    expect(css).toContain(".pkv-sync-button.is-secondary");
    expect(css).toContain("background: var(--pkv-bg-panel)");
    expect(css).toContain("box-shadow: 0 1px 2px rgba(15, 23, 42, 0.08)");
    expect(css).toContain(".pkv-sync-button.is-ghost");
    expect(css).toContain("background: var(--pkv-bg-panel)");
    expect(css).toContain("min-height: var(--pkv-compact-control-height)");
  });

  it("styles connected devices as a structured device list", () => {
    expect(css).toContain(".pkv-sync-device-card");
    expect(css).toContain(".pkv-sync-device-status");
    expect(css).toContain(".pkv-sync-device-name");
    expect(css).toContain(".pkv-sync-device-badge");
  });
});
