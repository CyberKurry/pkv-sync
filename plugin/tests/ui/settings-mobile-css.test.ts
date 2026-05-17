import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("settings mobile CSS", () => {
  const css = readFileSync(resolve(__dirname, "../../styles.css"), "utf8");

  it("adds top safe-area padding for Obsidian mobile settings chrome", () => {
    expect(css).toContain("body.is-mobile .pkv-sync-panel");
    expect(css).toContain(".pkv-sync-settings-host.is-mobile .pkv-sync-panel");
    expect(css).toContain("env(safe-area-inset-top)");
    expect(css).toContain("--pkv-mobile-top-offset");
    expect(css).toContain("--pkv-mobile-chrome-offset");
    expect(css).toContain(".pkv-sync-settings-host.is-phone");
  });
});
