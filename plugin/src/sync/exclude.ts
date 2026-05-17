export function isExcluded(path: string, globs: string[]): boolean {
  if (globs.length === 0) return false;
  for (const glob of globs) {
    const trimmed = glob.trim();
    if (!trimmed) continue;
    if (matchesGlob(path, trimmed)) return true;
  }
  return false;
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
