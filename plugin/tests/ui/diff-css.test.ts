import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("diff CSS", () => {
  const css = readFileSync(resolve(__dirname, "../../styles.css"), "utf8");
  const diffModal = readFileSync(resolve(__dirname, "../../src/ui/diff-modal.ts"), "utf8");
  const conflictModal = readFileSync(
    resolve(__dirname, "../../src/ui/conflict-resolve-modal.ts"),
    "utf8"
  );
  const historyModal = readFileSync(resolve(__dirname, "../../src/ui/history-modal.ts"), "utf8");

  it("caps the history modal width", () => {
    expect(css).toMatch(/\.modal\.pkvsync-modal-history\s*\{[\s\S]+?max-width:\s*720px/);
    expect(historyModal).toContain('this.modalEl.addClass("pkvsync-modal-history")');
  });

  it("gives split diff modals a GitHub-like wide viewport", () => {
    expect(css).toMatch(
      /\.modal\.pkvsync-modal-diff,\s*\.modal\.pkvsync-modal-conflict-resolve\s*\{[\s\S]+?width:\s*min\(98vw,\s*1680px\)/
    );
    expect(css).toMatch(
      /\.modal\.pkvsync-modal-diff,\s*\.modal\.pkvsync-modal-conflict-resolve\s*\{[\s\S]+?max-width:\s*min\(98vw,\s*1680px\)/
    );
    expect(diffModal).toContain('this.modalEl.addClass("pkvsync-modal-diff")');
    expect(conflictModal).toContain('this.modalEl.addClass("pkvsync-modal-conflict-resolve")');
  });

  it("keeps split diff columns readable instead of squeezing text", () => {
    expect(css).toMatch(
      /\.pkvsync-diff-split-header,\s*\.pkvsync-diff-split-row\s*\{[\s\S]+?grid-template-columns:\s*64px minmax\(560px,\s*1fr\) 64px minmax\(560px,\s*1fr\)/
    );
    expect(css).toMatch(/\.pkvsync-diff-split-header,\s*\.pkvsync-diff-split-row\s*\{[\s\S]+?min-width:\s*1248px/);
    expect(css).toMatch(/\.pkvsync-diff-split\s*\{[\s\S]+?overflow:\s*auto/);
    expect(css).toMatch(/\.pkvsync-diff-cell\s*\{[\s\S]+?white-space:\s*pre-wrap/);
    expect(css).toMatch(/\.pkvsync-diff-cell\s*\{[\s\S]+?overflow-wrap:\s*anywhere/);
  });

  it("marks split diff containers as table-like structures", () => {
    expect(diffModal).toContain('setAttr("role", "table")');
    expect(conflictModal).toContain('setAttr("role", "table")');
  });
});
