import { describe, expect, it, vi } from "vitest";
import { RollbackConfirmModal } from "../../src/ui/rollback-confirm-modal";
import { en } from "../../src/i18n/en";

function openedModal(onConfirm = vi.fn()): {
  modal: RollbackConfirmModal;
  input: HTMLInputElement;
  confirm: HTMLButtonElement;
  inputListener: () => void;
  clickListener: () => void;
  onConfirm: ReturnType<typeof vi.fn>;
} {
  const modal = new RollbackConfirmModal(
    {} as never,
    {
      vaultName: "Project Vault",
      commit: "1234567890abcdef",
      labels: en,
      onConfirm
    }
  );
  modal.open();

  const controls = modal as unknown as {
    confirmInput: HTMLInputElement | null;
    confirmButton: HTMLButtonElement | null;
  };
  const input = controls.confirmInput;
  const confirm = controls.confirmButton;
  if (!input || !confirm) throw new Error("modal controls were not rendered");

  const inputListener = vi
    .mocked(input.addEventListener)
    .mock.calls.find(([event]) => event === "input")?.[1] as (() => void) | undefined;
  const clickListener = vi
    .mocked(confirm.addEventListener)
    .mock.calls.find(([event]) => event === "click")?.[1] as (() => void) | undefined;
  if (!inputListener || !clickListener) throw new Error("modal listeners were not registered");

  return { modal, input, confirm, inputListener, clickListener, onConfirm };
}

describe("RollbackConfirmModal", () => {
  it("keeps confirm disabled for empty vault name", () => {
    const { input, confirm, inputListener } = openedModal();

    input.value = "";
    inputListener();

    expect(confirm.disabled).toBe(true);
  });

  it("keeps confirm disabled for the wrong vault name", () => {
    const { input, confirm, inputListener } = openedModal();

    input.value = "project vault";
    inputListener();

    expect(confirm.disabled).toBe(true);
  });

  it("enables confirm for the exact vault name", () => {
    const { input, confirm, inputListener } = openedModal();

    input.value = "Project Vault";
    inputListener();

    expect(confirm.disabled).toBe(false);
  });

  it("clicking confirm calls callback once", async () => {
    const onConfirm = vi.fn().mockResolvedValue(undefined);
    const { input, inputListener, clickListener } = openedModal(onConfirm);

    input.value = "Project Vault";
    inputListener();
    clickListener();
    clickListener();
    await Promise.resolve();

    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onConfirm).toHaveBeenCalledWith("Project Vault");
  });
});
