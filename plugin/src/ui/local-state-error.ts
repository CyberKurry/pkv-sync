import { format, type Strings } from "../i18n";
import type { DataCommitError } from "../plugin-store";
import { errorToMessage } from "../util";

type LocalStateLabels = Pick<
  Strings,
  "localStateSaveFailed" | "localStateRecoveryFailed" | "localStateApplyFailed"
>;

export function localStateErrorMessage(error: DataCommitError, labels: LocalStateLabels): string {
  const template = error.state === "restored"
    ? labels.localStateSaveFailed
    : error.state === "committed"
      ? labels.localStateApplyFailed
      : labels.localStateRecoveryFailed;
  return format(template, { reason: errorToMessage(error.original) });
}
