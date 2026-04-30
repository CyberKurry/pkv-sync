import { describe, expect, it } from "vitest";
import { statusText } from "../src/ui/status";

describe("statusText", () => {
  it("formats connected", () => {
    expect(statusText("connected")).toBe("PKV Sync: connected");
  });

  it("formats error detail", () => {
    expect(statusText("error", "401")).toBe("PKV Sync: error: 401");
  });
});
