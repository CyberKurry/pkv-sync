export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = Number.isFinite(bytes) ? Math.max(0, bytes) : 0;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  if (unit === 0 || Number.isInteger(value)) {
    return `${Math.trunc(value)} ${units[unit]}`;
  }
  return `${value.toFixed(1)} ${units[unit]}`;
}
