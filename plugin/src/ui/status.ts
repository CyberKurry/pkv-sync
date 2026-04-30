import type { Strings } from "../i18n";

export type SyncStatus =
  | "not_configured"
  | "connected"
  | "syncing"
  | "offline"
  | "error";

export type StatusLabels = Pick<
  Strings,
  | "statusConnected"
  | "statusNotConfigured"
  | "statusSyncing"
  | "statusOffline"
  | "statusError"
>;

const DEFAULT_STATUS_LABELS: StatusLabels = {
  statusConnected: "connected",
  statusNotConfigured: "not configured",
  statusSyncing: "syncing",
  statusOffline: "offline",
  statusError: "error"
};

export function statusText(
  status: SyncStatus,
  detail = "",
  labels: StatusLabels = DEFAULT_STATUS_LABELS
): string {
  const suffix = detail ? `: ${detail}` : "";
  switch (status) {
    case "not_configured":
      return `PKV Sync: ${labels.statusNotConfigured}`;
    case "connected":
      return `PKV Sync: ${labels.statusConnected}${suffix}`;
    case "syncing":
      return `PKV Sync: ${labels.statusSyncing}${suffix}`;
    case "offline":
      return `PKV Sync: ${labels.statusOffline}${suffix}`;
    case "error":
      return `PKV Sync: ${labels.statusError}${suffix}`;
  }
}
