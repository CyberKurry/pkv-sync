import { describe, expect, it, vi } from "vitest";
import {
  RestoreConfirmModal,
  type RestoreConfirmLabels
} from "../../src/ui/restore-confirm";

const labels: RestoreConfirmLabels = {
  restoreConfirmTitle: "Restore {fileName} from {commit}",
  restoreConfirmBody: "Snapshot {time} at {commit}",
  restoreUnsyncedWarning: "Unsynced local changes",
  restoreCancel: "Cancel",
  restoreConfirm: "Restore"
};

describe("RestoreConfirmModal", () => {
  it("renders replacement syntax literally in interpolated values", () => {
    const modal = new RestoreConfirmModal({
      app: {} as never,
      fileName: "$&-$'-$`-$$-{commit}.md",
      atCommitShort: "$&abc",
      atTimeRelative: "$'now",
      hasUnsyncedLocalChanges: false,
      labels,
      onConfirm: vi.fn()
    });

    modal.open();

    const contentEl = modal.contentEl as unknown as {
      createEl: ReturnType<typeof vi.fn>;
      createDiv: ReturnType<typeof vi.fn>;
    };

    expect(contentEl.createEl).toHaveBeenCalledWith("h2", {
      text: "Restore $&-$'-$`-$$-{commit}.md from $&abc"
    });
    expect(contentEl.createDiv).toHaveBeenCalledWith({
      cls: "pkvsync-restore-body",
      text: "Snapshot $'now at $&abc"
    });
  });
});
