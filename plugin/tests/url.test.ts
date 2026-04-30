import { describe, expect, it } from "vitest";
import { ServerUrlError, parseServerUrl } from "../src/url";

describe("parseServerUrl", () => {
  it("parses share URL with key path", () => {
    expect(parseServerUrl("https://sync.example.com/k_abc123/")).toEqual({
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_abc123"
    });
  });

  it("uses fallback key for plain URL", () => {
    expect(parseServerUrl("https://sync.example.com", "k_xyz")).toEqual({
      serverUrl: "https://sync.example.com",
      deploymentKey: "k_xyz"
    });
  });

  it("preserves subpath deployment", () => {
    expect(parseServerUrl("https://example.com/pkv", "k_1")).toEqual({
      serverUrl: "https://example.com/pkv",
      deploymentKey: "k_1"
    });
  });

  it("rejects missing key", () => {
    expect(() => parseServerUrl("https://x")).toThrow(ServerUrlError);
  });

  it("rejects invalid URL", () => {
    expect(() => parseServerUrl("not url", "k")).toThrow(ServerUrlError);
  });
});
