export type DiffLineKind = "context" | "add" | "del" | "hunk" | "meta";

export interface DiffLine {
  kind: DiffLineKind;
  text: string;
}

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

export function parseUnifiedDiff(patch: string): DiffLine[] {
  if (!patch) return [];
  return patch.split(/\r?\n/).map((text) => ({
    text,
    kind: classifyLine(text)
  }));
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
  return /^[ +\-]/.test(line) ? line.slice(1) : line;
}
