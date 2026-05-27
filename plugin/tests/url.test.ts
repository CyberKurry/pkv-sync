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

  it("rejects non-loopback http URLs", () => {
    expect(() => parseServerUrl("http://sync.example.com", "k_1")).toThrow(ServerUrlError);
  });

  it("allows loopback http URLs for local development", () => {
    expect(parseServerUrl("http://127.0.0.1:6710/k_local/")).toEqual({
      serverUrl: "http://127.0.0.1:6710",
      deploymentKey: "k_local"
    });
  });

  it("rejects http URLs on unspecified bind addresses", () => {
    expect(() => parseServerUrl("http://0.0.0.0:6710/k_local/")).toThrow(ServerUrlError);
    expect(() => parseServerUrl("http://[::]:6710/k_local/")).toThrow(ServerUrlError);
  });

  it("allows local development http URLs on IPv4-mapped loopback hosts", () => {
    expect(parseServerUrl("http://[::ffff:127.0.0.1]:6710/k_local/")).toEqual({
      serverUrl: "http://[::ffff:7f00:1]:6710",
      deploymentKey: "k_local"
    });
  });

  it("rejects missing key", () => {
    expect(() => parseServerUrl("https://x")).toThrow(ServerUrlError);
  });

  it("rejects invalid URL", () => {
    expect(() => parseServerUrl("not url", "k")).toThrow(ServerUrlError);
  });
});
