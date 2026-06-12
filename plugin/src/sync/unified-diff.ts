type DiffLineKind = "context" | "add" | "del" | "hunk" | "meta";

export type SideBySideDiffRowKind =
  | "context"
  | "add"
  | "del"
  | "modify"
  | "hunk"
  | "meta";

export interface SideBySideDiffRow {
  kind: SideBySideDiffRowKind;
  text?: string;
  leftLine?: number;
  rightLine?: number;
  leftText?: string;
  rightText?: string;
}

export function parseUnifiedDiffSideBySide(patch: string): SideBySideDiffRow[] {
  if (!patch) return [];
  const lines = patch.split(/\r?\n/);
  const rows: SideBySideDiffRow[] = [];
  let leftLine = 0;
  let rightLine = 0;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index] ?? "";
    const kind = classifyLine(line);

    if (kind === "meta") {
      rows.push({ kind: "meta", text: line });
      continue;
    }

    if (kind === "hunk") {
      const hunk = parseHunkHeader(line);
      leftLine = hunk?.leftStart ?? leftLine;
      rightLine = hunk?.rightStart ?? rightLine;
      rows.push({ kind: "hunk", text: line });
      continue;
    }

    if (kind === "context") {
      if (line.startsWith("\\")) {
        rows.push({ kind: "meta", text: line });
        continue;
      }
      rows.push({
        kind: "context",
        leftLine,
        rightLine,
        leftText: stripDiffPrefix(line),
        rightText: stripDiffPrefix(line)
      });
      leftLine += 1;
      rightLine += 1;
      continue;
    }

    if (kind === "del") {
      const deleted: string[] = [];
      const added: string[] = [];
      let cursor = index;
      while (classifyLine(lines[cursor] ?? "") === "del") {
        deleted.push(lines[cursor] ?? "");
        cursor += 1;
      }
      while (classifyLine(lines[cursor] ?? "") === "add") {
        added.push(lines[cursor] ?? "");
        cursor += 1;
      }
      const count = Math.max(deleted.length, added.length);
      for (let offset = 0; offset < count; offset += 1) {
        const deletedLine = deleted[offset];
        const addedLine = added[offset];
        if (deletedLine !== undefined && addedLine !== undefined) {
          rows.push({
            kind: "modify",
            leftLine,
            rightLine,
            leftText: stripDiffPrefix(deletedLine),
            rightText: stripDiffPrefix(addedLine)
          });
          leftLine += 1;
          rightLine += 1;
        } else if (deletedLine !== undefined) {
          rows.push({
            kind: "del",
            leftLine,
            leftText: stripDiffPrefix(deletedLine)
          });
          leftLine += 1;
        } else if (addedLine !== undefined) {
          rows.push({
            kind: "add",
            rightLine,
            rightText: stripDiffPrefix(addedLine)
          });
          rightLine += 1;
        }
      }
      index = cursor - 1;
      continue;
    }

    rows.push({
      kind: "add",
      rightLine,
      rightText: stripDiffPrefix(line)
    });
    rightLine += 1;
  }

  return rows;
}

function classifyLine(line: string): DiffLineKind {
  if (line.startsWith("@@")) return "hunk";
  if (line.startsWith("---") || line.startsWith("+++")) return "meta";
  if (line.startsWith("+")) return "add";
  if (line.startsWith("-")) return "del";
  return "context";
}

function parseHunkHeader(
  line: string
): { leftStart: number; rightStart: number } | null {
  const match = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
  if (!match) return null;
  return {
    leftStart: Number(match[1]),
    rightStart: Number(match[2])
  };
}

function stripDiffPrefix(line: string): string {
  return /^[ +-]/.test(line) ? line.slice(1) : line;
}

const LINE_DIFF_MAX_LINES = 1000;

export function lineDiffSideBySide(
  leftText: string,
  rightText: string
): { rows: SideBySideDiffRow[]; truncated: boolean } {
  const leftLines = leftText.split(/\r?\n/);
  const rightLines = rightText.split(/\r?\n/);
  const truncated =
    leftLines.length > LINE_DIFF_MAX_LINES ||
    rightLines.length > LINE_DIFF_MAX_LINES;
  const a = leftLines.slice(0, LINE_DIFF_MAX_LINES);
  const b = rightLines.slice(0, LINE_DIFF_MAX_LINES);
  const m = a.length;
  const n = b.length;
  const dp: number[][] = [];
  for (let i = 0; i <= m; i += 1) {
    dp.push(new Array(n + 1).fill(0));
  }
  for (let i = 1; i <= m; i += 1) {
    for (let j = 1; j <= n; j += 1) {
      if (a[i - 1] === b[j - 1]) {
        dp[i][j] = dp[i - 1][j - 1] + 1;
      } else {
        dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
      }
    }
  }
  type Op =
    | { kind: "eq"; left: string; right: string }
    | { kind: "del"; left: string }
    | { kind: "add"; right: string };
  const ops: Op[] = [];
  let i = m;
  let j = n;
  while (i > 0 && j > 0) {
    if (a[i - 1] === b[j - 1]) {
      ops.push({ kind: "eq", left: a[i - 1], right: b[j - 1] });
      i -= 1;
      j -= 1;
    } else if (dp[i - 1][j] >= dp[i][j - 1]) {
      ops.push({ kind: "del", left: a[i - 1] });
      i -= 1;
    } else {
      ops.push({ kind: "add", right: b[j - 1] });
      j -= 1;
    }
  }
  while (i > 0) {
    ops.push({ kind: "del", left: a[i - 1] });
    i -= 1;
  }
  while (j > 0) {
    ops.push({ kind: "add", right: b[j - 1] });
    j -= 1;
  }
  ops.reverse();

  const rows: SideBySideDiffRow[] = [];
  let leftNum = 1;
  let rightNum = 1;
  for (let k = 0; k < ops.length; k += 1) {
    const op = ops[k];
    if (op.kind === "eq") {
      rows.push({
        kind: "context",
        leftLine: leftNum,
        rightLine: rightNum,
        leftText: op.left,
        rightText: op.right
      });
      leftNum += 1;
      rightNum += 1;
    } else if (op.kind === "del") {
      const next = ops[k + 1];
      if (next && next.kind === "add") {
        rows.push({
          kind: "modify",
          leftLine: leftNum,
          rightLine: rightNum,
          leftText: op.left,
          rightText: next.right
        });
        leftNum += 1;
        rightNum += 1;
        k += 1;
      } else {
        rows.push({
          kind: "del",
          leftLine: leftNum,
          leftText: op.left
        });
        leftNum += 1;
      }
    } else {
      rows.push({
        kind: "add",
        rightLine: rightNum,
        rightText: op.right
      });
      rightNum += 1;
    }
  }

  return { rows, truncated };
}

export interface DiffSegment {
  text: string;
  changed: boolean;
}

const WORD_DIFF_CHANGE_RATIO_GUARD = 0.7;

function tokenizeForWordDiff(line: string): string[] {
  const tokens: string[] = [];
  const re =
    /([A-Za-z0-9_]+)|([\u3400-\u4dbf\u4e00-\u9fff\u3040-\u30ff\uac00-\ud7af])|(\s+)|(.)/gu;
  let match: RegExpExecArray | null;
  while ((match = re.exec(line)) !== null) {
    tokens.push(match[0]);
  }
  return tokens;
}

function mergeSegments(parts: DiffSegment[]): DiffSegment[] {
  const out: DiffSegment[] = [];
  for (const part of parts) {
    const last = out[out.length - 1];
    if (last && last.changed === part.changed) {
      last.text += part.text;
    } else {
      out.push({ ...part });
    }
  }
  return out;
}

function normalizeWordDiffWhitespace(parts: DiffSegment[]): DiffSegment[] {
  const normalized = parts.map((part) => ({ ...part }));
  for (let index = 1; index < normalized.length - 1; index += 1) {
    const part = normalized[index];
    if (
      !part.changed &&
      part.text.trim().length === 0 &&
      normalized[index - 1]?.changed &&
      normalized[index + 1]?.changed
    ) {
      part.changed = true;
    }
  }

  const merged = mergeSegments(normalized);
  for (let index = 1; index < merged.length; index += 1) {
    const part = merged[index];
    const previous = merged[index - 1];
    if (!part.changed || !previous || previous.changed) continue;
    const leadingWhitespace = /^\s+/.exec(part.text)?.[0] ?? "";
    if (!leadingWhitespace || leadingWhitespace.length === part.text.length) {
      continue;
    }
    previous.text += leadingWhitespace;
    part.text = part.text.slice(leadingWhitespace.length);
  }
  return merged.filter((part) => part.text.length > 0);
}

export function wordDiff(
  left: string,
  right: string
): { leftSegments: DiffSegment[]; rightSegments: DiffSegment[] } {
  if (left === right) {
    const same: DiffSegment[] = left ? [{ text: left, changed: false }] : [];
    return {
      leftSegments: same.map((segment) => ({ ...segment })),
      rightSegments: same.map((segment) => ({ ...segment }))
    };
  }

  const a = tokenizeForWordDiff(left);
  const b = tokenizeForWordDiff(right);
  const m = a.length;
  const n = b.length;
  const dp: number[][] = [];
  for (let i = 0; i <= m; i += 1) {
    dp.push(new Array(n + 1).fill(0));
  }
  for (let i = 1; i <= m; i += 1) {
    for (let j = 1; j <= n; j += 1) {
      dp[i][j] =
        a[i - 1] === b[j - 1]
          ? dp[i - 1][j - 1] + 1
          : Math.max(dp[i - 1][j], dp[i][j - 1]);
    }
  }

  const leftRev: DiffSegment[] = [];
  const rightRev: DiffSegment[] = [];
  let i = m;
  let j = n;
  while (i > 0 && j > 0) {
    if (a[i - 1] === b[j - 1]) {
      leftRev.push({ text: a[i - 1], changed: false });
      rightRev.push({ text: b[j - 1], changed: false });
      i -= 1;
      j -= 1;
    } else if (dp[i - 1][j] >= dp[i][j - 1]) {
      leftRev.push({ text: a[i - 1], changed: true });
      i -= 1;
    } else {
      rightRev.push({ text: b[j - 1], changed: true });
      j -= 1;
    }
  }
  while (i > 0) {
    leftRev.push({ text: a[i - 1], changed: true });
    i -= 1;
  }
  while (j > 0) {
    rightRev.push({ text: b[j - 1], changed: true });
    j -= 1;
  }

  const leftParts = leftRev.reverse();
  const rightParts = rightRev.reverse();
  const changedRatio = (parts: DiffSegment[]): number => {
    const contentParts = parts.filter((part) => part.text.trim().length > 0);
    if (contentParts.length === 0) return 0;
    return (
      contentParts.filter((part) => part.changed).length / contentParts.length
    );
  };

  if (
    changedRatio(leftParts) > WORD_DIFF_CHANGE_RATIO_GUARD ||
    changedRatio(rightParts) > WORD_DIFF_CHANGE_RATIO_GUARD
  ) {
    return {
      leftSegments: left ? [{ text: left, changed: true }] : [],
      rightSegments: right ? [{ text: right, changed: true }] : []
    };
  }

  return {
    leftSegments: normalizeWordDiffWhitespace(leftParts),
    rightSegments: normalizeWordDiffWhitespace(rightParts)
  };
}
