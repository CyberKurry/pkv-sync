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

  it("refuses valid multibyte escapes too: they fold onto the decoded name (R14-M1)", () => {
    // R12-2 pinned this as accepted, but the server stores the DECODED
    // name (decode_to_fixed_point): a literal %E6%96%87.md syncs under
    // 文.md — a sibling literally named 文.md then collapses onto the
    // same server path with independent index keys, and the pull
    // re-delivers the file under the decoded name. Any literal whose
    // fixed point differs from itself breaks the 1:1 local↔server path
    // mapping, so both are refused at scan time now.
    expect(shouldSyncPath("%E6%96%87.md")).toBe(false);
  });

  it("still accepts plain literal percent-free names", () => {
    expect(shouldSyncPath("plain.md")).toBe(true);
  });
});
