import { describe, expect, it } from "vitest";
import { errorToMessage, extensionOf } from "../src/util";

describe("plugin util helpers", () => {
  it("turns unknown errors into displayable messages", () => {
    expect(errorToMessage(new Error("boom"))).toBe("boom");
    expect(errorToMessage("plain")).toBe("plain");
    expect(errorToMessage(null)).toBe("null");
  });

  it("extracts lowercase file extensions from vault paths", () => {
    expect(extensionOf("Images/Photo.JPG")).toBe("jpg");
    expect(extensionOf("archive.tar.gz")).toBe("gz");
    expect(extensionOf("folder.name/LICENSE")).toBe("");
    expect(extensionOf("README")).toBe("");
  });
});
