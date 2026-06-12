import { describe, expect, it } from "vitest";
import { fillDiffCell } from "../../src/ui/diff-cells";

describe("fillDiffCell", () => {
  it("renders changed words as marked spans on modify rows", () => {
    const cell = new MockElement("div");

    fillDiffCell(
      cell as unknown as HTMLElement,
      {
        kind: "modify",
        leftLine: 1,
        rightLine: 1,
        leftText: "the quick fox",
        rightText: "the slow fox"
      },
      "right"
    );

    const marked = cell.querySelectorAll(".pkvsync-diff-word-changed");
    expect(marked.length).toBe(1);
    expect(marked[0]?.textContent).toBe("slow");
    expect(cell.textContent).toBe("the slow fox");
  });

  it("renders plain text for context rows", () => {
    const cell = new MockElement("div");

    fillDiffCell(
      cell as unknown as HTMLElement,
      {
        kind: "context",
        leftLine: 1,
        rightLine: 1,
        leftText: "x",
        rightText: "x"
      },
      "left"
    );

    expect(cell.querySelectorAll(".pkvsync-diff-word-changed").length).toBe(0);
    expect(cell.textContent).toBe("x");
  });
});

class MockElement {
  private readonly children: MockElement[] = [];
  private text = "";
  private readonly classes = new Set<string>();

  constructor(private readonly tag: string) {}

  createSpan(options: { cls?: string; text?: string } = {}): MockElement {
    const child = new MockElement("span");
    child.addClassNames(options.cls);
    child.text = options.text ?? "";
    this.children.push(child);
    return child;
  }

  appendText(text: string): void {
    this.children.push(MockElement.textNode(text));
  }

  setText(text: string): void {
    this.children.length = 0;
    this.text = text;
  }

  querySelectorAll(selector: string): MockElement[] {
    if (!selector.startsWith(".")) return [];
    const cls = selector.slice(1);
    const matches: MockElement[] = [];
    this.collectByClass(cls, matches);
    return matches;
  }

  get textContent(): string {
    return this.text + this.children.map((child) => child.textContent).join("");
  }

  private static textNode(text: string): MockElement {
    const node = new MockElement("#text");
    node.text = text;
    return node;
  }

  private addClassNames(cls?: string): void {
    for (const name of cls?.split(/\s+/) ?? []) {
      if (name) this.classes.add(name);
    }
  }

  private collectByClass(cls: string, matches: MockElement[]): void {
    if (this.classes.has(cls)) matches.push(this);
    for (const child of this.children) {
      child.collectByClass(cls, matches);
    }
  }
}
