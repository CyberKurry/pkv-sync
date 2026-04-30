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
