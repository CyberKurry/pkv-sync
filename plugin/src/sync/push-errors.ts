import { format, type Strings } from "../i18n";

export type PushFailure =
  | { kind: "hard_limit"; path: string; bytes: number; limit: number }
  | { kind: "remote_singleton"; path: string; bytes: number }
  | { kind: "retry_after"; deadline: number }
  | { kind: "retry_budget"; failures: number };

export type PushLabels = Pick<
  Strings,
  | "pushInlineTooLarge"
  | "pushProxyTooLarge"
  | "pushRetryLater"
  | "pushSplitBudgetExceeded"
>;

export class PushBatchError extends Error {
  /**
   * R7R3-3: the retry_after countdown is computed when the message is
   * READ, not when the error is constructed. The error is built the moment
   * a 413+Retry-After lands, but the user sees it seconds later (engine
   * catch -> status bar / Notice) — a construction-time "retries in Ns"
   * could already be expired, or show a stale 1s. A getter keeps the
   * message accurate at display time without changing the other kinds.
   */
  private static buildMessage(
    detail: PushFailure,
    labels: PushLabels
  ): string {
    if (detail.kind === "hard_limit") {
      return format(labels.pushInlineTooLarge, {
        path: detail.path,
        bytes: detail.bytes,
        limit: detail.limit
      });
    }
    if (detail.kind === "remote_singleton") {
      return format(labels.pushProxyTooLarge, {
        path: detail.path,
        bytes: detail.bytes
      });
    }
    if (detail.kind === "retry_after") {
      return format(labels.pushRetryLater, {
        seconds: Math.max(1, Math.ceil((detail.deadline - Date.now()) / 1000))
      });
    }
    return format(labels.pushSplitBudgetExceeded, {
      count: detail.failures
    });
  }

  constructor(
    public readonly detail: PushFailure,
    labels: PushLabels
  ) {
    // NOTE: Error's constructor installs `message` as an OWN property,
    // which shadows any prototype getter — so super() must be called
    // WITHOUT a message for the getter below to ever run. (Verified: a
    // getter defined alongside super(msg) is dead code.)
    super();
    this.name = "PushBatchError";
    this.labels = labels;
  }

  private readonly labels: PushLabels;

  /** Computed at read time so the retry_after countdown is accurate. */
  override get message(): string {
    return PushBatchError.buildMessage(this.detail, this.labels);
  }
}
