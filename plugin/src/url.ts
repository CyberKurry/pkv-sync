export interface ParsedServerUrl {
  serverUrl: string;
  deploymentKey: string;
}

export class ServerUrlError extends Error {}

export function parseServerUrl(input: string, fallbackKey = ""): ParsedServerUrl {
  const trimmed = input.trim();
  if (!trimmed) throw new ServerUrlError("Server URL is required");

  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    throw new ServerUrlError("Invalid server URL");
  }

  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new ServerUrlError("Server URL must use http or https");
  }
  if (!isLocalHttpAllowed(url)) {
    throw new ServerUrlError("Server URL must use https unless it points to localhost");
  }

  const segments = url.pathname.split("/").filter(Boolean);
  let deploymentKey = fallbackKey.trim();
  if (segments.length > 0 && /^k_[A-Za-z0-9]+$/.test(segments[0])) {
    deploymentKey = segments[0];
    url.pathname = "/";
  }
  if (!deploymentKey) throw new ServerUrlError("Deployment key is required");

  url.hash = "";
  url.search = "";
  url.pathname = url.pathname.replace(/\/+$/, "") || "/";
  const base = `${url.protocol}//${url.host}${url.pathname === "/" ? "" : url.pathname}`;
  return { serverUrl: base, deploymentKey };
}

export function isLocalHttpAllowed(url: URL): boolean {
  return url.protocol !== "http:" || isLoopbackHost(url.hostname);
}

function isLoopbackHost(hostname: string): boolean {
  const host = hostname.toLowerCase().replace(/^\[|\]$/g, "");
  if (
    host === "localhost" ||
    host.endsWith(".localhost") ||
    host === "::1"
  ) {
    return true;
  }
  const mapped = host.match(/^::ffff:(.+)$/);
  if (mapped?.[1] === "7f00:1") return true;
  if (mapped) return isLoopbackHost(mapped[1]);
  const octets = host.split(".");
  if (octets.length !== 4 || octets[0] !== "127") return false;
  return octets.every((part) => {
    if (!/^\d+$/.test(part)) return false;
    const value = Number(part);
    return value >= 0 && value <= 255 && String(value) === part;
  });
}
