export interface PushLimits {
  max_json_bytes: number;
  max_changes: number;
}

export const LEGACY_PUSH_LIMITS: Readonly<PushLimits> = Object.freeze({
  max_json_bytes: 2_097_152,
  max_changes: 1000
});

export const DEFAULT_PUSH_TARGET_BYTES = 1_048_576;
export const MAX_413_FAILURES = 16;
export const SINGLETON_RETRY_MS = 300_000;

function positiveInteger(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0
    ? value
    : fallback;
}

export function normalizePushLimits(raw: unknown): PushLimits {
  const value =
    raw !== null && typeof raw === "object"
      ? (raw as Record<string, unknown>)
      : {};
  return {
    max_json_bytes: positiveInteger(
      value.max_json_bytes,
      LEGACY_PUSH_LIMITS.max_json_bytes
    ),
    max_changes: Math.min(
      1000,
      positiveInteger(value.max_changes, LEGACY_PUSH_LIMITS.max_changes)
    )
  };
}
