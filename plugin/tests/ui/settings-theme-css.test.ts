import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("settings theme CSS", () => {
  const css = readFileSync(resolve(__dirname, "../../styles.css"), "utf8");

  it("defines hard light and dark theme overrides that work in any Obsidian theme", () => {
    // Both overrides exist and use color-scheme so the browser switches form
    // controls and scrollbars along with the colour palette.
    expect(css).toContain(".pkv-sync-settings-host.is-light-override");
    expect(css).toContain(".pkv-sync-settings-host.is-dark-override");
    expect(css).toContain("color-scheme: light");
    expect(css).toContain("color-scheme: dark");

    // Overrides set their own backgrounds rather than inheriting Obsidian's,
    // so they win in any base theme.
    expect(css).toMatch(/\.pkv-sync-settings-host\.is-light-override\s*\{[\s\S]+?--pkv-bg-panel:/);
    expect(css).toMatch(/\.pkv-sync-settings-host\.is-dark-override\s*\{[\s\S]+?--pkv-bg-panel:/);
  });

  it("defines compact aligned controls for dense settings actions", () => {
    // Two control heights — full and compact — used by buttons, inputs, and
    // the textarea inside vault settings.
    expect(css).toContain("--pkv-control-h");
    expect(css).toContain("--pkv-control-h-sm");
    expect(css).toContain(".pkv-sync-textarea");
    expect(css).toContain(".pkv-sync-allowlist-actions");
    expect(css).toContain(".pkv-sync-vault-actions .pkv-sync-button");
    expect(css).toContain("height: var(--pkv-control-h-sm)");
  });

  it("keeps the language selector wide enough for localized labels", () => {
    expect(css).toContain(".pkv-sync-language-select");
    // Whatever the exact min-width number, the selector must declare one so
    // longer localized labels (繁體中文, 한국어) do not overflow.
    expect(css).toMatch(/\.pkv-sync-language-select[\s\S]+?min-width:\s*\d+px/);
    // The narrow compact variant from the old design must not creep back.
    expect(css).not.toContain(".pkv-sync-select-wrap.is-compact");
    expect(css).not.toContain("width: 58px");
  });

  it("renders theme mode as a single visible cycle button", () => {
    expect(css).toContain(".pkv-sync-theme-button");
    expect(css).toContain(".pkv-sync-theme-icon");
    expect(css).toContain(".pkv-sync-theme-label");
    expect(css).toContain("[data-theme-mode=\"dark\"]");
    // The select-based variant must stay gone.
    expect(css).not.toContain(".pkv-sync-theme-select");
  });

  it("makes manual theme overrides win over the current Obsidian app theme", () => {
    // The override selectors are not nested under `body.theme-*`, so they
    // apply regardless of the Obsidian app theme.
    expect(css).not.toMatch(/body\.theme-light\s+\.pkv-sync-settings-host\.is-dark-override/);
    expect(css).not.toMatch(/body\.theme-dark\s+\.pkv-sync-settings-host\.is-light-override/);
    expect(css).toMatch(/^\.pkv-sync-settings-host\.is-light-override\s*\{/m);
    expect(css).toMatch(/^\.pkv-sync-settings-host\.is-dark-override\s*\{/m);
  });

  it("renders primary, secondary, and ghost actions as distinct buttons", () => {
    expect(css).toContain(".pkv-sync-button.is-primary");
    expect(css).toContain(".pkv-sync-button.is-secondary");
    expect(css).toContain(".pkv-sync-button.is-ghost");
    // All buttons share the unified control height token.
    expect(css).toMatch(/\.pkv-sync-button[\s\S]+?height:\s*var\(--pkv-control-h\)/);
  });

  it("styles connected devices as a structured device list", () => {
    expect(css).toContain(".pkv-sync-device-card");
    expect(css).toContain(".pkv-sync-device-status");
    expect(css).toContain(".pkv-sync-device-name");
    expect(css).toContain(".pkv-sync-device-badge");
  });
});
