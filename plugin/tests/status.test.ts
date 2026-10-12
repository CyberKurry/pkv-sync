import { describe, expect, it } from "vitest";
import { en } from "../src/i18n/en";
import { statusText } from "../src/ui/status";

describe("statusText", () => {
  it("formats connected", () => {
    expect(statusText("connected", "", en)).toBe("PKV Sync: connected");
  });

  it("formats error detail", () => {
    expect(statusText("error", "401", en)).toBe("PKV Sync: error: 401");
  });
});
