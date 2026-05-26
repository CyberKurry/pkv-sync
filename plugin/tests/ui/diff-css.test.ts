import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("diff CSS", () => {
  const css = readFileSync(resolve(__dirname, "../../styles.css"), "utf8");

  it("gives split diff modals a GitHub-like wide viewport", () => {
    expect(css).toMatch(
      /\.modal:has\(\.pkvsync-diff-modal\),\s*\.modal:has\(\.pkvsync-conflict-resolve-modal\)\s*\{[\s\S]+?width:\s*min\(96vw,\s*1280px\)/
    );
    expect(css).toMatch(
      /\.modal:has\(\.pkvsync-diff-modal\),\s*\.modal:has\(\.pkvsync-conflict-resolve-modal\)\s*\{[\s\S]+?max-width:\s*min\(96vw,\s*1280px\)/
    );
  });

  it("keeps split diff columns readable instead of squeezing text", () => {
    expect(css).toMatch(
      /\.pkvsync-diff-split-header,\s*\.pkvsync-diff-split-row\s*\{[\s\S]+?grid-template-columns:\s*56px minmax\(420px,\s*1fr\) 56px minmax\(420px,\s*1fr\)/
    );
    expect(css).toMatch(/\.pkvsync-diff-split\s*\{[\s\S]+?overflow:\s*auto/);
    expect(css).toMatch(/\.pkvsync-diff-cell\s*\{[\s\S]+?white-space:\s*pre-wrap/);
    expect(css).toMatch(/\.pkvsync-diff-cell\s*\{[\s\S]+?overflow-wrap:\s*anywhere/);
  });
});
