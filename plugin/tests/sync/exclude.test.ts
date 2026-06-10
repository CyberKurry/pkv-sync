import { describe, expect, it } from "vitest";
import { createPathMatcher, type PathAcceptsOptions } from "../../src/sync/exclude";

function pathAccepts(path: string, opts: PathAcceptsOptions): boolean {
  return createPathMatcher(opts)(path);
}

function isExcluded(path: string, globs: string[]): boolean {
  return !pathAccepts(path, {
    userExcludes: globs,
    userAllowlist: ["**"]
  });
}

describe("isExcluded", () => {
  it("returns false for empty globs", () => {
    expect(isExcluded("foo.md", [])).toBe(false);
  });

  it("matches *.tmp", () => {
    expect(isExcluded("foo.tmp", ["*.tmp"])).toBe(true);
    expect(isExcluded("foo.md", ["*.tmp"])).toBe(false);
    expect(isExcluded("dir/foo.tmp", ["*.tmp"])).toBe(true);
  });

  it("matches *.log", () => {
    expect(isExcluded("debug.log", ["*.log"])).toBe(true);
    expect(isExcluded("error.LOG", ["*.log"])).toBe(false);
  });

  it("matches build/**", () => {
    expect(isExcluded("build/x/y.js", ["build/**"])).toBe(true);
    expect(isExcluded("build/output.js", ["build/**"])).toBe(true);
    expect(isExcluded("src/build.md", ["build/**"])).toBe(false);
  });

  it("does not match directory name alone with build/**", () => {
    expect(isExcluded("build", ["build/**"])).toBe(false);
  });

  it("matches **/__pycache__/**", () => {
    expect(isExcluded("__pycache__/foo.pyc", ["**/__pycache__/**"])).toBe(true);
    expect(isExcluded("src/__pycache__/foo.pyc", ["**/__pycache__/**"])).toBe(true);
    expect(isExcluded("src/__pycache__/sub/foo.pyc", ["**/__pycache__/**"])).toBe(true);
    expect(isExcluded("src/main.py", ["**/__pycache__/**"])).toBe(false);
  });

  it("matches ? single char", () => {
    expect(isExcluded("file.a", ["file.?"])).toBe(true);
    expect(isExcluded("file.ab", ["file.?"])).toBe(false);
  });

  it("matches [abc] char class", () => {
    expect(isExcluded("file.a", ["file.[abc]"])).toBe(true);
    expect(isExcluded("file.d", ["file.[abc]"])).toBe(false);
  });

  it("treats regex escapes inside char classes as literals", () => {
    expect(isExcluded("file.d", ["file.[\\d]"])).toBe(true);
    expect(isExcluded("file.5", ["file.[\\d]"])).toBe(false);
  });

  it("skips empty glob entries", () => {
    expect(isExcluded("foo.md", ["", "  ", "\t"])).toBe(false);
  });

  it("handles multiple globs", () => {
    const globs = ["*.tmp", "*.log", "build/**"];
    expect(isExcluded("foo.tmp", globs)).toBe(true);
    expect(isExcluded("debug.log", globs)).toBe(true);
    expect(isExcluded("build/x.js", globs)).toBe(true);
    expect(isExcluded("src/main.ts", globs)).toBe(false);
  });

  it("handles dotfiles with .*", () => {
    expect(isExcluded(".env", [".*"])).toBe(true);
    expect(isExcluded(".gitignore", [".*"])).toBe(true);
    expect(isExcluded("normal.md", [".*"])).toBe(false);
  });

  it("handles paths with multiple segments", () => {
    expect(isExcluded("a/b/c/d.tmp", ["*.tmp"])).toBe(true);
    expect(isExcluded("a/b/c/d.md", ["*.tmp"])).toBe(false);
  });

  it("handles **/node_modules/**", () => {
    expect(isExcluded("foo/node_modules/bar", ["**/node_modules/**"])).toBe(true);
    expect(isExcluded("node_modules/foo", ["**/node_modules/**"])).toBe(true);
    expect(isExcluded("src/app.ts", ["**/node_modules/**"])).toBe(false);
  });

  it("matches *.ext at any depth", () => {
    expect(isExcluded("deep/nested/path/file.bak", ["*.bak"])).toBe(true);
  });

  it("handles Unicode filenames", () => {
    expect(isExcluded("文档.tmp", ["*.tmp"])).toBe(true);
    expect(isExcluded("文档.md", ["*.tmp"])).toBe(false);
  });

  it("matches exact filename", () => {
    expect(isExcluded("Thumbs.db", ["Thumbs.db"])).toBe(true);
    expect(isExcluded("folder/Thumbs.db", ["Thumbs.db"])).toBe(false);
  });

  it("matches dir/* for files inside dir recursively", () => {
    expect(isExcluded("dist/main.js", ["dist/*"])).toBe(true);
    expect(isExcluded("dist/sub/main.js", ["dist/*"])).toBe(true);
    expect(isExcluded("other/main.js", ["dist/*"])).toBe(false);
  });

  it("matches **/*.bak at any depth", () => {
    expect(isExcluded("file.bak", ["**/*.bak"])).toBe(true);
    expect(isExcluded("a/b/file.bak", ["**/*.bak"])).toBe(true);
    expect(isExcluded("file.md", ["**/*.bak"])).toBe(false);
  });

  it("handles *.tar.gz", () => {
    expect(isExcluded("archive.tar.gz", ["*.tar.gz"])).toBe(true);
    expect(isExcluded("archive.zip", ["*.tar.gz"])).toBe(false);
  });

  it("handles trailing ** without slash", () => {
    expect(isExcluded("foo", ["foo**"])).toBe(true);
    expect(isExcluded("foobar", ["foo**"])).toBe(true);
    expect(isExcluded("fo", ["foo**"])).toBe(false);
  });

  it("handles pattern with special regex chars", () => {
    expect(isExcluded("file.+ext", ["file.+ext"])).toBe(true);
    expect(isExcluded("file.ext", ["file.+ext"])).toBe(false);
  });

  it("handles pattern with parentheses", () => {
    expect(isExcluded("file(1).txt", ["file(1).txt"])).toBe(true);
    expect(isExcluded("file1.txt", ["file(1).txt"])).toBe(false);
  });

  it("handles unclosed bracket as literal", () => {
    expect(isExcluded("file[abc", ["file[abc"])).toBe(true);
    expect(isExcluded("filea", ["file[abc"])).toBe(false);
  });

  it("matches **/ at start matching any prefix", () => {
    expect(isExcluded("deep/nested/logs/app.log", ["**/logs/app.log"])).toBe(true);
    expect(isExcluded("logs/app.log", ["**/logs/app.log"])).toBe(true);
    expect(isExcluded("other/app.log", ["**/logs/app.log"])).toBe(false);
  });

  it("matches path with multiple wildcards", () => {
    expect(isExcluded("src/test/file.spec.ts", ["src/*/*.spec.ts"])).toBe(true);
    expect(isExcluded("src/a/b/file.spec.ts", ["src/*/*.spec.ts"])).toBe(true);
  });

  it("handles glob with only **", () => {
    expect(isExcluded("any/path/here", ["**"])).toBe(true);
    expect(isExcluded("file", ["**"])).toBe(true);
  });

  it("matches coverage/**", () => {
    expect(isExcluded("coverage/index.html", ["coverage/**"])).toBe(true);
    expect(isExcluded("coverage/lcov-report/main.js", ["coverage/**"])).toBe(true);
    expect(isExcluded("src/coverage.md", ["coverage/**"])).toBe(false);
  });

  it("handles consecutive ? wildcards", () => {
    expect(isExcluded("ab", ["??"])).toBe(true);
    expect(isExcluded("abc", ["??"])).toBe(false);
    expect(isExcluded("a", ["??"])).toBe(false);
  });

  it("handles multiple char class ranges", () => {
    expect(isExcluded("file.a", ["file.[a-c]"])).toBe(true);
    expect(isExcluded("file.b", ["file.[a-c]"])).toBe(true);
    expect(isExcluded("file.d", ["file.[a-c]"])).toBe(false);
  });

  it("matches * wildcard against any char including /", () => {
    expect(isExcluded("a/b", ["*"])).toBe(true);
    expect(isExcluded("a", ["*"])).toBe(true);
  });

  it("? does not match /", () => {
    expect(isExcluded("/", ["?"])).toBe(false);
    expect(isExcluded("a", ["?"])).toBe(true);
  });

  it("handles glob pattern .DS_Store", () => {
    expect(isExcluded(".DS_Store", [".DS_Store"])).toBe(true);
    expect(isExcluded("folder/.DS_Store", [".DS_Store"])).toBe(false);
  });
});

describe("createPathMatcher", () => {
  it("matches pathAccepts behavior for repeated calls", () => {
    const opts = {
      userExcludes: ["*.tmp", "private/**"],
      userAllowlist: [".obsidian/plugins/**"]
    };
    const matcher = createPathMatcher(opts);
    const paths = [
      "notes/today.md",
      "notes/draft.tmp",
      "private/secret.md",
      ".obsidian/plugins/pkv-sync/data.json",
      ".obsidian/workspace.json"
    ];

    for (const path of paths) {
      expect(matcher(path)).toBe(pathAccepts(path, opts));
      expect(matcher(path)).toBe(pathAccepts(path, opts));
    }
  });
});
