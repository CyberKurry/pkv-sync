import { describe, expect, it, vi } from "vitest";
import { strings } from "../../src/i18n";
import { EmptyScanConfirmModal } from "../../src/ui/empty-scan-confirm";

/**
 * Minimal element stub following the connect-confirm.test.ts StubElement
 * pattern: the component renders through Obsidian's createEl/createDiv
 * extensions, which a real DOM does not provide, so the harness — not the
 * component — supplies them.
 */
class StubElement {
  children: StubElement[] = [];
  private readonly clickListeners: Array<() => void> = [];
  text = "";
  cls = "";
  disabled = false;

  constructor(readonly tag: string) {}

  setText(next: string): void {
    this.text = next;
  }

  empty(): void {
    this.children = [];
    this.text = "";
  }

  createDiv(options: { cls?: string } = {}): StubElement {
    return this.appendChild("div", options);
  }

  createEl(
    tag: string,
    options: { text?: string; cls?: string } = {}
  ): StubElement {
    return this.appendChild(tag, options);
  }

  addEventListener(event: string, listener: () => void): void {
    if (event === "click") this.clickListeners.push(listener);
  }

  click(): void {
    // Real DOM buttons swallow clicks while disabled.
    if (this.disabled) return;
    for (const listener of [...this.clickListeners]) listener();
  }

  get textContent(): string {
    return [this.text, ...this.children.map((child) => child.textContent)]
      .filter(Boolean)
      .join(" ");
  }

  querySelectorAll(selector: string): StubElement[] {
    const tag = selector.trim().toLowerCase();
    return this.children.flatMap((child) => [
      ...(child.tag === tag ? [child] : []),
      ...child.querySelectorAll(selector)
    ]);
  }

  private appendChild(
    tag: string,
    options: { text?: string; cls?: string }
  ): StubElement {
    const child = new StubElement(tag);
    child.text = options.text ?? "";
    child.cls = options.cls ?? "";
    this.children.push(child);
    return child;
  }
}

function openEmptyScanModal(
  indexEntries: number,
  onAccept: () => void
): EmptyScanConfirmModal {
  const modal = new EmptyScanConfirmModal(
    {} as never,
    strings(),
    indexEntries,
    onAccept
  );
  modal.titleEl = new StubElement("h2") as never;
  modal.contentEl = new StubElement("div") as never;
  modal.onOpen();
  return modal;
}

describe("EmptyScanConfirmModal (D3 breaker)", () => {
  it("renders the held index-entry count in the warning body", () => {
    const modal = openEmptyScanModal(42, () => undefined);

    expect(modal.titleEl.textContent).toContain(
      strings().emptyScanConfirmTitle
    );
    expect(modal.contentEl.textContent).toContain("42");
  });

  it("cancel closes without arming the breaker", () => {
    const onAccept = vi.fn();
    const modal = openEmptyScanModal(3, onAccept);
    const buttons = modal.contentEl.querySelectorAll("button");
    expect(buttons).toHaveLength(2);

    buttons[1].click(); // cancel

    expect(onAccept).not.toHaveBeenCalled();
  });

  it("fires onAccept exactly once for a double click", () => {
    const onAccept = vi.fn();
    const modal = openEmptyScanModal(3, onAccept);
    const buttons = modal.contentEl.querySelectorAll("button");

    buttons[0].click();
    buttons[0].click();

    // The accept callback is sync, so no waitFor is needed.
    expect(onAccept).toHaveBeenCalledTimes(1);
  });
});
