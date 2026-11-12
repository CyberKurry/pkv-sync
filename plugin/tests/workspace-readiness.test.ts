import { describe, expect, it } from "vitest";
import { engineStartAllowed } from "../src/workspace-readiness";

describe("workspace readiness gate (0b)", () => {
  it("blocks engine rebuilds until the workspace layout is ready", () => {
    expect(engineStartAllowed({ layoutReady: false })).toBe(false);
  });
  it("allows rebuilds once layoutReady is true (covers runtime enable)", () => {
    expect(engineStartAllowed({ layoutReady: true })).toBe(true);
  });
});
