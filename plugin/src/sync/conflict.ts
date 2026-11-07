function safeDeviceName(name: string): string {
  return (
    name
      .trim()
      .replace(/[^A-Za-z0-9_-]+/g, "-")
      .replace(/-+/g, "-")
      .replace(/^-|-$/g, "") || "device"
  );
}

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

/**
 * 8 chars of [A-Za-z0-9], matching the server's merge-apply nonce
 * (uuid v4 simple, first 8). Always-on since the R13-2 fix: the plugin
 * used to append a nonce ONLY on an in-memory same-second collision, so
 * two same-second sidecars across a plugin reload silently overwrote
 * each other. An always-on nonce makes every sidecar path unique by
 * construction; `parseConflictPath` accepts both nonce and nonce-less
 * (legacy) names.
 */
function nonce8(): string {
  const bytes = new Uint8Array(6);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(36).padStart(2, "0"))
    .join("")
    .slice(0, 8)
    .replace(/[^A-Za-z0-9]/g, "0")
    .padEnd(8, "0");
}

export function conflictPath(
  original: string,
  deviceName: string,
  date = new Date()
): string {
  const stamp = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(
    date.getDate()
  )}-${pad(date.getHours())}${pad(date.getMinutes())}${pad(
    date.getSeconds()
  )}`;
  const device = safeDeviceName(deviceName);
  const nonce = nonce8();
  const slash = original.lastIndexOf("/");
  const dir = slash >= 0 ? original.slice(0, slash + 1) : "";
  const file = slash >= 0 ? original.slice(slash + 1) : original;
  const dot = file.lastIndexOf(".");
  if (dot <= 0) {
    return `${dir}${file}.conflict-${stamp}-${nonce}-${device}`;
  }
  return `${dir}${file.slice(0, dot)}.conflict-${stamp}-${nonce}-${device}${file.slice(dot)}`;
}
