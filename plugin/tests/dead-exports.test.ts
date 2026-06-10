import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

function source(path: string): string {
  return readFileSync(resolve(__dirname, `../src/${path}`), "utf8");
}

describe("plugin dead export cleanup", () => {
  it("does not expose unused helper APIs from production modules", () => {
    expect(source("sync/exclude.ts")).not.toMatch(
      /export function (isExcluded|pathAccepts)\b/
    );
    expect(source("sync/index-store.ts")).not.toMatch(
      /export function deletedFiles\b/
    );
    expect(source("sync/unified-diff.ts")).not.toMatch(
      /export (type DiffLineKind|interface DiffLine|function parseUnifiedDiff)\b/
    );
    expect(source("ui/diff-modal.ts")).not.toMatch(
      /export function diffLineClass\b/
    );
    expect(source("services/update-check.ts")).not.toMatch(
      /export function (resolvePluginAssetPath|extractSha256)\b/
    );
  });

  it("keeps internally used helpers private", () => {
    expect(source("sync/hash.ts")).not.toMatch(/export function toHex\b/);
    expect(source("ui/conflict-resolve-modal.ts")).not.toMatch(
      /export function mergeMarkerLineClass\b/
    );
    expect(source("services/update-check.ts")).not.toMatch(
      /export function extractSha256\b/
    );
    expect(source("i18n/index.ts")).not.toMatch(
      /export const languageBundles\b/
    );
    expect(source("sync/migrate-from-obsidian-sync.ts")).not.toMatch(
      /export function fileSize\b/
    );
    expect(source("sync/conflict-files.ts")).not.toMatch(
      /export interface ConflictFileReader\b/
    );
  });
});
