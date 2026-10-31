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
  constructor(
    public readonly detail: PushFailure,
    labels: PushLabels,
    now = Date.now()
  ) {
    const message =
      detail.kind === "hard_limit"
        ? format(labels.pushInlineTooLarge, {
            path: detail.path,
            bytes: detail.bytes,
            limit: detail.limit
          })
        : detail.kind === "remote_singleton"
          ? format(labels.pushProxyTooLarge, {
              path: detail.path,
              bytes: detail.bytes
            })
          : detail.kind === "retry_after"
            ? format(labels.pushRetryLater, {
                seconds: Math.max(
                  1,
                  Math.ceil((detail.deadline - now) / 1000)
                )
              })
            : format(labels.pushSplitBudgetExceeded, {
                count: detail.failures
              });
    super(message);
    this.name = "PushBatchError";
  }
}
