import { describe, expect, it } from "vitest";
import { conflictPath } from "../../src/sync/conflict";
import {
  isConflictPath,
  originalPathFor,
  parseConflictPath,
  sidecarRoleFor
} from "../../src/sync/conflict-files";

const d = new Date(2026, 3, 25, 14, 30, 22);

function sidecarRe(device: string, ext = ""): RegExp {
  return new RegExp(
    `^note\.conflict-2026-04-25-143022-[A-Za-z0-9]{8}-${device}${ext}$`
  );
}

describe("conflictPath", () => {
  it("inserts before extension with an 8-char nonce", () => {
    expect(conflictPath("note.md", "iPhone", d)).toMatch(sidecarRe("iPhone", ".md"));
  });

  it("preserves folder", () => {
    expect(conflictPath("folder/note.md", "Phone X", d)).toMatch(
      /^folder\/note\.conflict-2026-04-25-143022-[A-Za-z0-9]{8}-Phone-X\.md$/
    );
  });

  it("handles no extension", () => {
    expect(conflictPath("README", "d", d)).toMatch(
      /^README\.conflict-2026-04-25-143022-[A-Za-z0-9]{8}-d$/
    );
  });

  it("never collides inside one second (always-on nonce)", () => {
    const paths = new Set(
      Array.from({ length: 200 }, () => conflictPath("note.md", "phone", d))
    );
    // R13-2 red: the old format stamped only seconds, so every path was
    // identical; the nonce makes each unique.
    expect(paths.size).toBe(200);
  });

  it("round-trips: every produced path parses back to the original (R13-1)", () => {
    for (const original of [
      "note.md",
      "folder/note.md",
      "README",
      "folder/note.markdown.md",
      "weird name with spaces.md",
      "dotted.name.md",
      "folder/dotted.name.md"
    ]) {
      const cpath = conflictPath(original, "iPhone", d);
      expect(isConflictPath(cpath), cpath).toBe(true);
      expect(originalPathFor(cpath), cpath).toBe(original);
    }
  });
});

describe("parseConflictPath (single grammar, R13-1)", () => {
  it("parses legacy nonce-less sidecars", () => {
    const legacy = "note.conflict-2026-05-16-143000-phone.md";
    expect(isConflictPath(legacy)).toBe(true);
    expect(originalPathFor(legacy)).toBe("note.md");
    expect(sidecarRoleFor(legacy)).toBe("local");
  });

  it("parses server nonce sidecars", () => {
    const serverStyle = "note.conflict-2026-05-16-143000-ab12cd34-phone.md";
    const parsed = parseConflictPath(serverStyle);
    expect(parsed?.originalPath).toBe("note.md");
    expect(parsed?.nonce).toBe("ab12cd34");
    expect(parsed?.device).toBe("phone");
    expect(sidecarRoleFor(serverStyle)).toBe("local");
  });

  it("keeps the remote role on both nonce and legacy forms", () => {
    expect(
      sidecarRoleFor("note.conflict-2026-05-16-143000-remote.md")
    ).toBe("remote");
    expect(
      sidecarRoleFor("note.conflict-2026-05-16-143000-ab12cd34-remote.md")
    ).toBe("remote");
  });

  it("rejects non-sidecar names", () => {
    expect(isConflictPath("note.md")).toBe(false);
    expect(isConflictPath("report.conflict-2024.md")).toBe(false);
    expect(isConflictPath("note.conflict-2026-05-16-143000.md")).toBe(false);
  });
});
