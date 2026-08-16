import { describe, expect, it } from "vitest";
import {
  ServerUrlError,
  isFetchableServerUrl,
  parseServerUrl
} from "../src/url";

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

  it("rejects non-localhost 127/8 http URLs", () => {
    expect(() => parseServerUrl("http://127.0.0.2:6710/k_local/")).toThrow(ServerUrlError);
    expect(() => parseServerUrl("http://127.1.2.3:6710/k_local/")).toThrow(ServerUrlError);
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

describe("isFetchableServerUrl", () => {
  it("accepts https URLs", () => {
    expect(isFetchableServerUrl("https://sync.example.com")).toBe(true);
    expect(isFetchableServerUrl("https://example.com/pkv")).toBe(true);
  });

  it("accepts http only for loopback hosts", () => {
    expect(isFetchableServerUrl("http://localhost:6710")).toBe(true);
    expect(isFetchableServerUrl("http://127.0.0.1:6710")).toBe(true);
    expect(isFetchableServerUrl("http://[::1]:6710")).toBe(true);
    expect(isFetchableServerUrl("http://[::ffff:127.0.0.1]:6710")).toBe(true);
  });

  it("rejects http for LAN hosts", () => {
    expect(isFetchableServerUrl("http://192.168.1.10:6710")).toBe(false);
    expect(isFetchableServerUrl("http://sync.example.com")).toBe(false);
  });

  it("rejects non-http protocols", () => {
    expect(isFetchableServerUrl("javascript:alert(1)")).toBe(false);
    expect(isFetchableServerUrl("file:///etc/passwd")).toBe(false);
    expect(isFetchableServerUrl("ftp://example.com")).toBe(false);
  });

  it("rejects garbage and empty input", () => {
    expect(isFetchableServerUrl("not a url")).toBe(false);
    expect(isFetchableServerUrl("")).toBe(false);
    expect(isFetchableServerUrl("   ")).toBe(false);
  });
});
