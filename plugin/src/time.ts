export const DEFAULT_TIMEZONE = "Asia/Shanghai";

export const TIMEZONE_OPTIONS = [
  { value: "Asia/Shanghai", label: "Asia/Shanghai" },
  { value: "UTC", label: "UTC" },
  { value: "Asia/Tokyo", label: "Asia/Tokyo" },
  { value: "Asia/Hong_Kong", label: "Asia/Hong_Kong" },
  { value: "Asia/Singapore", label: "Asia/Singapore" },
  { value: "America/Los_Angeles", label: "America/Los_Angeles" },
  { value: "America/New_York", label: "America/New_York" },
  { value: "Europe/London", label: "Europe/London" },
  { value: "Europe/Berlin", label: "Europe/Berlin" },
  { value: "Australia/Sydney", label: "Australia/Sydney" }
];

export function normalizeTimezone(value: string | null | undefined): string {
  const timezone = value?.trim() || DEFAULT_TIMEZONE;
  try {
    new Intl.DateTimeFormat("en-US", { timeZone: timezone }).format(new Date(0));
    return timezone;
  } catch {
    return DEFAULT_TIMEZONE;
  }
}

export function formatUnixSeconds(
  timestamp: number | null | undefined,
  timezone: string
): string {
  if (timestamp === null || timestamp === undefined) return "";
  const date = new Date(timestamp * 1000);
  if (Number.isNaN(date.getTime())) return String(timestamp);
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: normalizeTimezone(timezone),
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
    hourCycle: "h23"
  }).formatToParts(date);
  const value = (type: string): string =>
    parts.find((part) => part.type === type)?.value ?? "00";
  return `${value("year")}-${value("month")}-${value("day")} ${value("hour")}:${value("minute")}:${value("second")}`;
}
