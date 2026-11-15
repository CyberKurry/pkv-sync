import { describe, expect, it } from "vitest";
import { shouldSyncPath } from "../../src/sync/vault-adapter";

/**
 * R15-1: the plugin's percent-decode loop and the server's
 * `decode_to_fixed_point` (server/src/storage/path.rs) are two producers of
 * ONE contract — "the stored path is the fixed point of decoding". They
 * disagreed at the round budget:
 *
 * - server: up to `MAX_DECODE_ROUNDS` (4) decode rounds inside the loop,
 *   then ONE final round outside it. Stable after ≤4 rounds → accepted and
 *   stored at the fixed point; still mutating after the final round →
 *   `PercentEncodingTooDeep` → 400 `invalid_path`.
 * - plugin (before this fix): 4 rounds, then `return false` — budget
 *   exhaustion meant ACCEPT. So a chain needing exactly 4 rounds slipped
 *   past R14-M1's sibling-overwrite refusal (it IS stored at a decoded name
 *   server-side, colliding with the sibling that spells it directly), and a
 *   chain needing 5+ rounds was pushed into a deterministic 400 the client
 *   had already classified as syncable (the R15-2 whole-vault wedge).
 *
 * The table below is the contract in one place: the plugin must reach the
 * same verdict as the server for every depth, and the verdict for any
 * literal is "refuse", because a literal that decodes at all maps to a
 * different server path than its local key.
 */

/** Decodes to a fixed point the way the server does; reports the depth. */
function fixedPoint(encoded: string): { value: string; rounds: number } {
  let current = encoded;
  let rounds = 0;
  for (;;) {
    const next = decodeURIComponent(current);
    if (next === current) return { value: current, rounds };
    current = next;
    rounds += 1;
    if (rounds > 8) throw new Error(`no fixed point for ${encoded}`);
  }
}

/**
 * What the server does with this input, mirroring `decode_to_fixed_point`:
 * `Ok(fixed point)` when it stabilizes within the 4-round loop, otherwise
 * one final round decides between `Ok` (the 4th round was already stable)
 * and `Err(TooDeep)`.
 */
function serverVerdict(encoded: string): "accepted" | "too_deep" {
  return fixedPoint(encoded).rounds <= 4 ? "accepted" : "too_deep";
}

describe("R15-1 decode depth agrees with the server at every round count", () => {
  // depth 1..6 of the same `a%41.md` → `aA.md` chain.
  const chain = [
    "a%41.md",
    "a%2541.md",
    "a%252541.md",
    "a%25252541.md",
    "a%2525252541.md",
    "a%252525252541.md"
  ];

  it("pins the decode depth of the fixture chain", () => {
    // Guard the table itself: if this drifts, the assertions below stop
    // testing the depths they claim to.
    expect(chain.map((path) => fixedPoint(path).rounds)).toEqual([1, 2, 3, 4, 5, 6]);
    expect(chain.map((path) => fixedPoint(path).value)).toEqual([
      "aA.md",
      "aA.md",
      "aA.md",
      "aA.md",
      "aA.md",
      "aA.md"
    ]);
  });

  it.each(chain)("refuses %s at every depth the server folds it", (path) => {
    // Depth ≤4: the server stores it at aA.md, so the literal collapses
    // onto a sibling named aA.md (R14-M1's silent overwrite). Depth ≥5:
    // the server refuses with 400, so pushing it wedges the vault
    // (R15-2). Both directions require the plugin to refuse.
    expect(shouldSyncPath(path)).toBe(false);
  });

  it("mirrors the server verdict for the depth-4 and depth-5 boundary", () => {
    // The exact boundary this finding is about. If either side ever moves
    // its budget, this line tells the next reader which one moved.
    expect(serverVerdict("a%25252541.md")).toBe("accepted");
    expect(serverVerdict("a%2525252541.md")).toBe("too_deep");
    expect(shouldSyncPath("a%25252541.md")).toBe(false);
    expect(shouldSyncPath("a%2525252541.md")).toBe(false);
  });

  it("keeps the decoded spelling and escape-free names syncable", () => {
    expect(shouldSyncPath("aA.md")).toBe(true);
    expect(shouldSyncPath("100%.md")).toBe(true);
    expect(shouldSyncPath("notes/todo.md")).toBe(true);
  });

  it("refuses a deep chain whose 4th round lands on a dot root", () => {
    // The per-round shape checks must keep running through the final
    // check, not only inside the loop.
    expect(shouldSyncPath("%252525252Eobsidian/notes.md")).toBe(false);
  });
});
