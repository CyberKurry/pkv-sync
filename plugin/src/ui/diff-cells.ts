import type { SideBySideDiffRow } from "../sync/unified-diff";
import { wordDiff } from "../sync/unified-diff";

export function fillDiffCell(
  cell: HTMLElement,
  row: SideBySideDiffRow,
  side: "left" | "right"
): void {
  const text = side === "left" ? row.leftText : row.rightText;
  if (row.kind !== "modify" || text === undefined) {
    cell.setText(text ?? "");
    return;
  }

  const diff = wordDiff(row.leftText ?? "", row.rightText ?? "");
  const segments = side === "left" ? diff.leftSegments : diff.rightSegments;
  for (const segment of segments) {
    if (segment.changed) {
      cell.createSpan({
        cls: "pkvsync-diff-word-changed",
        text: segment.text
      });
    } else {
      cell.appendText(segment.text);
    }
  }
}
