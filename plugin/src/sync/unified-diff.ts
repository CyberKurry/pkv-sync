export type DiffLineKind = "context" | "add" | "del" | "hunk" | "meta";

export interface DiffLine {
  kind: DiffLineKind;
  text: string;
}

export function parseUnifiedDiff(patch: string): DiffLine[] {
  if (!patch) return [];
  return patch.split(/\r?\n/).map((text) => ({
    text,
    kind: classifyLine(text)
  }));
}

function classifyLine(line: string): DiffLineKind {
  if (line.startsWith("@@")) return "hunk";
  if (line.startsWith("---") || line.startsWith("+++")) return "meta";
  if (line.startsWith("+")) return "add";
  if (line.startsWith("-")) return "del";
  return "context";
}
