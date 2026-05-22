import { describe, expect, it } from "vitest";
import { mergeMarkerLineClass } from "../../src/ui/conflict-resolve-modal";

describe("conflict resolve modal helpers", () => {
  it("maps merge marker lines to stable CSS classes", () => {
    expect(mergeMarkerLineClass("<<<<<<< local")).toBe(
      "pkvsync-merge-marker-local"
    );
    expect(mergeMarkerLineClass("=======")).toBe(
      "pkvsync-merge-marker-separator"
    );
    expect(mergeMarkerLineClass(">>>>>>> remote")).toBe(
      "pkvsync-merge-marker-remote"
    );
    expect(mergeMarkerLineClass("resolved text")).toBe(
      "pkvsync-merge-marker-content"
    );
  });
});
