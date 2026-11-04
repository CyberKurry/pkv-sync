import { describe, expect, it, vi } from "vitest";
import { PKVSyncSettingTab } from "../../src/ui/settings-tab";

/**
 * R9-Dup-5 (the one latent bug in the redundancy batch): the sync-now
 * icon was injected by comparing the button's display text against the
 * CURRENT locale's syncNowButton string — any copy tweak, or a second
 * button sharing that label, silently dropped the icon. The icon must
 * come from an explicit renderButton parameter instead.
 */
describe("renderButton icon parameter (R9-Dup-5)", () => {
  interface MockEl {
    cls: string;
    textContent: string;
    children: MockEl[];
    addEventListener: ReturnType<typeof vi.fn>;
    empty(): void;
    createSpan(opts?: { cls?: string; text?: string }): MockEl;
  }

  function makeParent(): {
    createEl: (_tag: string, opts?: { cls?: string; text?: string }) => MockEl;
    created: MockEl[];
  } {
    const created: MockEl[] = [];
    const newEl = (opts?: { cls?: string; text?: string }): MockEl => {
      const el: MockEl = {
        cls: opts?.cls ?? "",
        textContent: opts?.text ?? "",
        children: [],
        addEventListener: vi.fn(),
        empty(): void {
          el.children = [];
          el.textContent = "";
        },
        createSpan(spanOpts?: { cls?: string; text?: string }): MockEl {
          const span = newEl(spanOpts);
          el.children.push(span);
          return span;
        }
      };
      created.push(el);
      return el;
    };
    return {
      created,
      createEl(_tag: string, opts?: { cls?: string; text?: string }): MockEl {
        return newEl(opts);
      }
    };
  }

  function renderButton(
    text: string,
    icon?: string
  ): { button: MockEl; iconSetOn: unknown } {
    const parent = makeParent();
    const tab = Object.create(PKVSyncSettingTab.prototype) as unknown as Record<
      string,
      unknown
    >;
    tab.plugin = {
      text: () => ({ syncNowButton: "SYNC-NOW-LOCALIZED" })
    };
    const proto = PKVSyncSettingTab.prototype as unknown as Record<
      string,
      unknown
    >;
    const fn = (proto.renderButton as (
      this: Record<string, unknown>,
      parent: unknown,
      text: string,
      variant: string,
      onClick: () => void,
      icon?: string
    ) => MockEl).bind(tab);
    const button = fn(
      parent,
      text,
      "primary",
      () => undefined,
      icon
    );
    return { button, iconSetOn: [] };
  }

  it("injects the icon span when the icon argument is passed, regardless of label", () => {
    const { button } = renderButton("whatever label", "refresh-cw");
    expect(button.cls).toContain("pkv-sync-button");
    expect(button.children.length).toBe(2);
    expect(button.children[0].cls).toBe("pkv-sync-button-icon");
    expect(button.children[1].textContent).toBe("whatever label");
  });

  it("does not inject an icon from label matching alone", () => {
    // Even the exact localized syncNowButton label gets no icon unless
    // the parameter says so — the old text comparison died with this.
    const { button } = renderButton("SYNC-NOW-LOCALIZED");
    expect(button.children.length).toBe(0);
    expect(button.textContent).toBe("SYNC-NOW-LOCALIZED");
  });

  it("plain buttons render text-only", () => {
    const { button } = renderButton("Login");
    expect(button.textContent).toBe("Login");
    expect(button.children.length).toBe(0);
  });
});
