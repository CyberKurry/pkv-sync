const HARD_EXCLUDE_GLOBS = [
  ".obsidian/workspace.json",
  ".obsidian/workspace-mobile.json",
  ".obsidian/workspaces.json",
  ".obsidian/cache/**",
  ".git/**",
  ".trash/**",
  ".conflict-*",
  "*.lock",
  "*.tmp"
];

export interface PathAcceptsOptions {
  userExcludes: string[];
  userAllowlist: string[];
}

type PathMatcher = (path: string) => boolean;

export function createPathMatcher(opts: PathAcceptsOptions): PathMatcher {
  const hardExclude = createExcludeMatcher(HARD_EXCLUDE_GLOBS);
  const userExcludes = createExcludeMatcher(opts.userExcludes);
  const userAllowlist = createExcludeMatcher(opts.userAllowlist);

  return (path: string) => {
    if (isHardExcluded(path, hardExclude)) return false;

    if (isHiddenPath(path) && !userAllowlist(path)) return false;

    return !userExcludes(path);
  };
}

function createExcludeMatcher(globs: string[]): PathMatcher {
  const regexes = nonEmptyGlobs(globs).map(compileGlob);
  if (regexes.length === 0) return () => false;
  return (path: string) => regexes.some((regex) => regex.test(path));
}

function isHardExcluded(path: string, hardExclude: PathMatcher): boolean {
  if (hardExclude(path)) return true;
  return path.includes("/.git/")
    || path.includes("/.trash/")
    || path.includes("/.conflict-");
}

function isHiddenPath(path: string): boolean {
  return path.startsWith(".") || path.includes("/.");
}

function nonEmptyGlobs(globs: string[]): string[] {
  return globs.filter((glob) => glob.trim().length > 0);
}

function compileGlob(pattern: string): RegExp {
  return new RegExp(`^${globToRegex(pattern.trim())}$`);
}

function globToRegex(pattern: string): string {
  let result = "";
  let i = 0;
  while (i < pattern.length) {
    const ch = pattern[i];
    if (ch === "*" && i + 1 < pattern.length && pattern[i + 1] === "*") {
      if (i + 2 < pattern.length && pattern[i + 2] === "/") {
        result += "(?:.+/)?";
        i += 3;
      } else {
        result += ".*";
        i += 2;
      }
    } else if (ch === "*") {
      result += ".*";
      i++;
    } else if (ch === "?") {
      result += "[^/]";
      i++;
    } else if (ch === "[") {
      const end = pattern.indexOf("]", i + 1);
      if (end === -1) {
        result += escapeRegex(ch);
        i++;
      } else {
        result += charClassToRegex(pattern.slice(i + 1, end));
        i = end + 1;
      }
    } else {
      result += escapeRegex(ch);
      i++;
    }
  }
  return result;
}

function charClassToRegex(content: string): string {
  if (content.length === 0) return "\\[\\]";

  let index = 0;
  let prefix = "";
  if ((content[0] === "!" || content[0] === "^") && content.length > 1) {
    prefix = "^";
    index = 1;
  }

  let body = "";
  while (index < content.length) {
    body += escapeCharClass(content[index], body.length === 0);
    index++;
  }

  return `[${prefix}${body}]`;
}

function escapeCharClass(ch: string, first: boolean): string {
  if (ch === "\\") return "\\\\";
  if (ch === "]") return "\\]";
  if (ch === "^" && first) return "\\^";
  return ch;
}

function escapeRegex(ch: string): string {
  if (/[.+^${}()|[\]\\]/.test(ch)) return `\\${ch}`;
  return ch;
}
