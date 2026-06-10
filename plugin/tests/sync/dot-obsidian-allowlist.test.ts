import { describe, expect, it } from "vitest";
import { createPathMatcher, type PathAcceptsOptions } from "../../src/sync/exclude";

function pathAccepts(path: string, opts: PathAcceptsOptions): boolean {
  return createPathMatcher(opts)(path);
}

describe("pathAccepts", () => {
  it("accepts normal paths without excludes", () => {
    expect(pathAccepts("notes/today.md", { userExcludes: [], userAllowlist: [] })).toBe(true);
  });

  it("rejects hidden paths without an allowlist", () => {
    expect(pathAccepts(".obsidian/themes/my.css", { userExcludes: [], userAllowlist: [] })).toBe(false);
  });

  it("accepts allowlisted .obsidian theme files", () => {
    expect(
      pathAccepts(".obsidian/themes/my.css", {
        userExcludes: [],
        userAllowlist: [".obsidian/themes/**"]
      })
    ).toBe(true);
  });

  it("rejects .obsidian plugin files that are not allowlisted", () => {
    expect(
      pathAccepts(".obsidian/plugins/foo/main.js", {
        userExcludes: [],
        userAllowlist: [".obsidian/themes/**"]
      })
    ).toBe(false);
  });

  it("rejects hard excluded workspace files even when allowlisted", () => {
    expect(
      pathAccepts(".obsidian/workspace.json", {
        userExcludes: [],
        userAllowlist: [".obsidian/**"]
      })
    ).toBe(false);
  });

  it("rejects other hard excludes even when allowlisted", () => {
    const hardExcluded = [
      ".obsidian/cache/index.json",
      ".git/config",
      "notes/.git/config",
      ".trash/deleted.md",
      "notes/.trash/deleted.md",
      ".conflict-note.md",
      "notes/.conflict-note.md",
      "notes/theme.lock",
      "notes/cache.tmp"
    ];

    for (const path of hardExcluded) {
      expect(
        pathAccepts(path, {
          userExcludes: [],
          userAllowlist: ["**"]
        })
      ).toBe(false);
    }
  });

  it("accepts nested hidden paths when the visible parent is allowlisted", () => {
    expect(
      pathAccepts(".claude/agents/.cache/file.json", {
        userExcludes: [],
        userAllowlist: [".claude/agents/**"]
      })
    ).toBe(true);
  });

  it("rejects user excluded normal paths", () => {
    expect(
      pathAccepts("notes/draft.tmp", {
        userExcludes: ["*.tmp"],
        userAllowlist: []
      })
    ).toBe(false);
  });

  it("ignores empty exclude and allowlist entries", () => {
    expect(
      pathAccepts("notes/today.md", {
        userExcludes: ["", "  "],
        userAllowlist: ["", "  "]
      })
    ).toBe(true);
  });

  it("keeps invalid exclude patterns compatible with isExcluded", () => {
    expect(
      pathAccepts("file[abc", {
        userExcludes: ["file[abc"],
        userAllowlist: []
      })
    ).toBe(false);
  });
});
