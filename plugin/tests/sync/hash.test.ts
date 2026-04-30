import { describe, expect, it } from "vitest";
import { sha256Text, toHex } from "../../src/sync/hash";

describe("hash", () => {
  it("hex encodes bytes", () => {
    expect(toHex(new Uint8Array([0, 15, 255]))).toBe("000fff");
  });

  it("sha256Text matches known vector", async () => {
    expect(await sha256Text("hello")).toBe(
      "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
  });
});
