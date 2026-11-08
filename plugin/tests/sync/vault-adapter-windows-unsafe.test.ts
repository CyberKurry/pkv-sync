import { describe, expect, it } from "vitest";
import { shouldSyncPath } from "../../src/sync/vault-adapter";

/**
 * R14-M3 (mirror of the server's is_windows_unsafe_component): the
 * server refuses Windows-reserved device names, components with a colon
 * and trailing dots/spaces on EVERY platform — a Windows client joining
 * later could not materialize them. Without a scan-side prefilter the
 * plugin pushes such files into a deterministic 400 invalid_path every
 * pass, wedging the whole vault's push stream (the same no-quarantine
 * mechanism as R14-H2).
 */
describe("R14-M3 Windows-unsafe components are refused at scan time", () => {
  it.each(["aux.md", "nul", "com1.md", "lpt9.txt", "sub/prn.md"])(
    "refuses reserved device name %s",
    (path) => {
      expect(shouldSyncPath(path)).toBe(false);
    },
  );

  it("refuses reserved names with extensions (CON.txt rule)", () => {
    expect(shouldSyncPath("con.md.backup")).toBe(false);
  });

  it.each(["note ", "note.", "sub/trailing "])(
    "refuses trailing dot/space component %s",
    (path) => {
      expect(shouldSyncPath(path)).toBe(false);
    },
  );

  it("refuses components containing a colon", () => {
    expect(shouldSyncPath("a:b.md")).toBe(false);
  });

  it("keeps names that merely CONTAIN a reserved stem", () => {
    expect(shouldSyncPath("auxiliary.md")).toBe(true);
    expect(shouldSyncPath("notes/com1c.md")).toBe(true);
  });

  it("keeps ordinary paths", () => {
    expect(shouldSyncPath("notes/todo.md")).toBe(true);
  });
});
