import { describe, expect, it } from "vitest";
import { guessMime } from "../../src/sync/mime";

describe("sync MIME helpers", () => {
  it("detects common binary MIME types case-insensitively", () => {
    expect(guessMime("Images/Photo.JPG")).toBe("image/jpeg");
    expect(guessMime("attachments/report.pdf")).toBe("application/pdf");
  });

  it("returns undefined for paths without a known extension", () => {
    expect(guessMime("notes/archive.unknown")).toBeUndefined();
    expect(guessMime("LICENSE")).toBeUndefined();
  });
});
