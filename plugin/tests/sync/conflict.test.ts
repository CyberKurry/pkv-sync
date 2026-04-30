import { describe, expect, it } from "vitest";
import { conflictPath } from "../../src/sync/conflict";

const d = new Date(2026, 3, 25, 14, 30, 22);

describe("conflictPath", () => {
  it("inserts before extension", () => {
    expect(conflictPath("note.md", "iPhone", d)).toBe(
      "note.conflict-2026-04-25-143022-iPhone.md"
    );
  });

  it("preserves folder", () => {
    expect(conflictPath("folder/note.md", "Phone X", d)).toBe(
      "folder/note.conflict-2026-04-25-143022-Phone-X.md"
    );
  });

  it("handles no extension", () => {
    expect(conflictPath("README", "d", d)).toBe(
      "README.conflict-2026-04-25-143022-d"
    );
  });
});
