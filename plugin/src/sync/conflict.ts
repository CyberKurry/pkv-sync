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
  const slash = original.lastIndexOf("/");
  const dir = slash >= 0 ? original.slice(0, slash + 1) : "";
  const file = slash >= 0 ? original.slice(slash + 1) : original;
  const dot = file.lastIndexOf(".");
  if (dot <= 0) return `${dir}${file}.conflict-${stamp}-${device}`;
  return `${dir}${file.slice(0, dot)}.conflict-${stamp}-${device}${file.slice(
    dot
  )}`;
}
