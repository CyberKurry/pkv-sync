/**
 * R8-Dup-2: the modal button builder shared by diff-modal, history-modal
 * and restore-confirm (previously three byte-identical private copies).
 */
export function modalButton(
  parent: HTMLElement,
  text: string,
  onClick: () => void | Promise<void>
): HTMLButtonElement {
  const button = parent.createEl("button", {
    cls: "pkvsync-button",
    text
  });
  button.addEventListener("click", () => void onClick());
  return button;
}
