import { describe, expect, it } from "vitest";
import { shouldSyncPath } from "../../src/sync/vault-adapter";

/**
 * R12-2 (twin of the server's InvalidUtf8Escape): an escape whose decoded
 * bytes are not valid UTF-8 (`a%FFb.md`) used to pass the plugin's scan
 * filter (decodeURIComponent throws → catch returned false = "safe"),
 * while the server decoded it lossily to `a\uFFFDb.md` — the file was
 * renamed on the server and the two ends never converged. Both ends now
 * refuse: the plugin at scan time, the server at normalize time.
 */
describe("R12-2 invalid-UTF-8 percent escapes are refused on both ends", () => {
  it("refuses %FF escapes (not valid UTF-8 bytes)", () => {
    expect(shouldSyncPath("a%FFb.md")).toBe(false);
  });

  it("refuses truncated multibyte escapes (%E6%96 alone)", () => {
    expect(shouldSyncPath("note%E6%96.md")).toBe(false);
  });

  it("still accepts valid multibyte escapes (%E6%96%87 = 文)", () => {
    expect(shouldSyncPath("%E6%96%87.md")).toBe(true);
  });

  it("still accepts plain literal percent-free names", () => {
    expect(shouldSyncPath("plain.md")).toBe(true);
  });
});
