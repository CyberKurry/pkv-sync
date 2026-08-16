import { describe, expect, it, vi } from "vitest";
import { en } from "../../src/i18n/en";
import {
  DeleteConflictsConfirmModal,
  type DeleteConflictsConfirmParams
} from "../../src/ui/delete-conflicts-confirm";

type ClickableElement = {
  addEventListener: ReturnType<typeof vi.fn>;
};

type ElementMock = {
  createEl: ReturnType<typeof vi.fn> & {
    mock: {
      calls: Array<[string, { text?: string; cls?: string }]>;
      results: Array<{ value: ClickableElement }>;
    };
  };
  createDiv: ReturnType<typeof vi.fn> & {
    mock: {
      calls: Array<[{ cls?: string; text?: string }]>;
      results: Array<{ value: ElementMock }>;
    };
  };
};

function buildParams(
  overrides: Partial<DeleteConflictsConfirmParams> = {}
): DeleteConflictsConfirmParams {
  return {
    app: {} as never,
    count: 3,
    labels: en,
    onConfirm: vi.fn(),
    onClose: vi.fn(),
    ...overrides
  };
}

function contentElOf(modal: DeleteConflictsConfirmModal): ElementMock {
  return modal.contentEl as unknown as ElementMock;
}

function actionsElement(contentEl: ElementMock): ElementMock {
  for (const [index, call] of contentEl.createDiv.mock.calls.entries()) {
    if (call[0]?.cls === "pkvsync-delete-conflicts-actions") {
      const actions = contentEl.createDiv.mock.results[index]?.value;
      if (!actions) throw new Error("Actions element missing");
      return actions;
    }
  }
  throw new Error("Actions element not created");
}

function buttonByLabel(parent: ElementMock, text: string): ClickableElement {
  for (const [index, call] of parent.createEl.mock.calls.entries()) {
    const [tag, options] = call;
    if (tag === "button" && options?.text === text) {
      const button = parent.createEl.mock.results[index]?.value;
      if (!button) throw new Error(`Button mock missing: ${text}`);
      return button;
    }
  }
  throw new Error(`Button not found: ${text}`);
}

function click(element: ClickableElement): void {
  for (const [, handler] of element.addEventListener.mock.calls) {
    handler();
  }
}

describe("DeleteConflictsConfirmModal", () => {
  it("renders the localized title and count in the message", () => {
    const modal = new DeleteConflictsConfirmModal(buildParams());

    modal.open();

    const contentEl = contentElOf(modal);
    expect(contentEl.createEl).toHaveBeenCalledWith("h2", {
      text: en.deleteConflictsConfirmTitle
    });
    expect(contentEl.createDiv).toHaveBeenCalledWith({
      cls: "pkvsync-delete-conflicts-body",
      text: "Move 3 conflict file(s) to the system trash? Original notes are kept."
    });
  });

  it("does not confirm deletion when cancelled", () => {
    const onConfirm = vi.fn();
    const onClose = vi.fn();
    const modal = new DeleteConflictsConfirmModal(
      buildParams({ onConfirm, onClose })
    );

    modal.open();
    click(buttonByLabel(actionsElement(contentElOf(modal)), "Cancel"));

    expect(onConfirm).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("requires an explicit confirm click before deletion runs", async () => {
    const onConfirm = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    const modal = new DeleteConflictsConfirmModal(
      buildParams({ onConfirm, onClose })
    );

    modal.open();
    expect(onConfirm).not.toHaveBeenCalled();

    click(buttonByLabel(actionsElement(contentElOf(modal)), "Delete"));
    await vi.waitFor(() => expect(onConfirm).toHaveBeenCalledTimes(1));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
