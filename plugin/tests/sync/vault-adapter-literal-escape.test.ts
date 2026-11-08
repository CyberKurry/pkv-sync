import { describe, expect, it } from "vitest";
import { shouldSyncPath } from "../../src/sync/vault-adapter";

/**
 * R14-M1 (twin of the server's idempotent decode contract): the server
 * decodes every pushed path to its fixed point before storing it, so a
 * local file literally named `a%41.md` lives at `aA.md` on the server.
 * A sibling literally named `aA.md` maps to the SAME server path — two
 * index keys, one server file, silent mutual overwrite with no conflict
 * sidecar. The plugin must refuse any literal whose fixed-point decode
 * differs from itself, exactly like the folding rule the server already
 * applies to what it receives.
 */
describe("R14-M1 literal percent escapes that decode are refused", () => {
  it("refuses a%41.md (decodes to the sibling-shaped aA.md)", () => {
    expect(shouldSyncPath("a%41.md")).toBe(false);
  });

  it("refuses double-encoded forms (%2541 decodes in rounds)", () => {
    expect(shouldSyncPath("a%2541.md")).toBe(false);
  });

  it("keeps the decoded spelling itself syncable", () => {
    expect(shouldSyncPath("aA.md")).toBe(true);
  });

  it("keeps a bare percent without hex (no escape, nothing decodes)", () => {
    expect(shouldSyncPath("100%.md")).toBe(true);
  });

  it("keeps ordinary percent-free paths", () => {
    expect(shouldSyncPath("notes/todo.md")).toBe(true);
  });
});
