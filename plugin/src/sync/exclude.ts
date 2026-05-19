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

export function isExcluded(path: string, globs: string[]): boolean {
  if (globs.length === 0) return false;
  for (const glob of globs) {
    const trimmed = glob.trim();
    if (!trimmed) continue;
    if (matchesGlob(path, trimmed)) return true;
  }
  return false;
}

export function pathAccepts(path: string, opts: PathAcceptsOptions): boolean {
  if (isHardExcluded(path)) return false;

  if (isHiddenPath(path)) {
    const allowlist = nonEmptyGlobs(opts.userAllowlist);
    if (allowlist.length === 0 || !isExcluded(path, allowlist)) return false;
  }

  return !isExcluded(path, opts.userExcludes);
}

function isHardExcluded(path: string): boolean {
  if (isExcluded(path, HARD_EXCLUDE_GLOBS)) return true;
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

function matchesGlob(path: string, pattern: string): boolean {
  const regexStr = globToRegex(pattern);
  const regex = new RegExp(`^${regexStr}$`);
  return regex.test(path);
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
        result += pattern.slice(i, end + 1);
        i = end + 1;
      }
    } else {
      result += escapeRegex(ch);
      i++;
    }
  }
  return result;
}

function escapeRegex(ch: string): string {
  if (/[.+^${}()|[\]\\]/.test(ch)) return `\\${ch}`;
  return ch;
}
