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

const TIMEZONE_VALIDATION_CACHE = new Map<string, string>();
const UNIX_SECONDS_FORMATTER_CACHE = new Map<string, Intl.DateTimeFormat>();

function normalizeTimezone(value: string | null | undefined): string {
  const timezone = value?.trim() || DEFAULT_TIMEZONE;
  const cached = TIMEZONE_VALIDATION_CACHE.get(timezone);
  if (cached) return cached;
  try {
    new Intl.DateTimeFormat("en-US", { timeZone: timezone }).format(new Date(0));
    TIMEZONE_VALIDATION_CACHE.set(timezone, timezone);
    return timezone;
  } catch {
    TIMEZONE_VALIDATION_CACHE.set(timezone, DEFAULT_TIMEZONE);
    return DEFAULT_TIMEZONE;
  }
}

function getUnixSecondsFormatter(timezone: string): Intl.DateTimeFormat {
  const normalizedTimezone = normalizeTimezone(timezone);
  const cached = UNIX_SECONDS_FORMATTER_CACHE.get(normalizedTimezone);
  if (cached) return cached;
  const formatter = new Intl.DateTimeFormat("en-US", {
    timeZone: normalizedTimezone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
    hourCycle: "h23"
  });
  UNIX_SECONDS_FORMATTER_CACHE.set(normalizedTimezone, formatter);
  return formatter;
}

export function formatUnixSeconds(
  timestamp: number | null | undefined,
  timezone: string
): string {
  if (timestamp === null || timestamp === undefined) return "";
  const date = new Date(timestamp * 1000);
  if (Number.isNaN(date.getTime())) return String(timestamp);
  const parts = getUnixSecondsFormatter(timezone).formatToParts(date);
  const value = (type: string): string =>
    parts.find((part) => part.type === type)?.value ?? "00";
  return `${value("year")}-${value("month")}-${value("day")} ${value("hour")}:${value("minute")}:${value("second")}`;
}

export function formatRelativeUnixSeconds(
  timestamp: number | null | undefined,
  nowSeconds = Math.floor(Date.now() / 1000)
): string {
  if (timestamp === null || timestamp === undefined) return "";
  if (!Number.isFinite(timestamp) || !Number.isFinite(nowSeconds)) return "";
  const diff = Math.max(0, Math.floor(nowSeconds - timestamp));
  if (diff < 60) return diff <= 1 ? "just now" : `${diff} sec ago`;
  const minutes = Math.floor(diff / 60);
  if (minutes < 60) return minutes === 1 ? "1 min ago" : `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return hours === 1 ? "1 hr ago" : `${hours} hr ago`;
  const days = Math.floor(hours / 24);
  return days === 1 ? "1 day ago" : `${days} days ago`;
}

export function formatDetailedUnixSeconds(
  timestamp: number | null | undefined,
  timezone: string
): string {
  return formatUnixSeconds(timestamp, timezone).replace(/-/g, "/");
}
