// @vitest-environment jsdom
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const STATIC = resolve(process.cwd(), "../server/static");
const RENDERER = resolve(STATIC, "admin-markdown.js");
type PreviewWindow = Window & {
  marked?: { parse: (source: string, options?: object) => string };
  DOMPurify?: { sanitize: (source: string, options?: object) => unknown };
};
const previewWindow = window as PreviewWindow;

function browserScript(source: string): void {
  new Function("window", "self", "globalThis", "module", "exports", "define", source)
    .call(window, window, window, window, undefined, undefined, undefined);
}

function viewer(source: string): HTMLElement {
  document.body.innerHTML = `<section data-markdown-viewer
    data-properties-label="Document properties" data-image-label="Image"
    data-preview-error-text="Preview unavailable; read the source."
    data-files-base="/admin/vaults/v/files/">
    <div data-markdown-tabs role="tablist" aria-label="View mode" hidden>
      <button id="file-preview-tab" type="button" role="tab" aria-selected="false"
        aria-controls="file-preview-pane" data-markdown-mode="preview">Preview</button>
      <button id="file-source-tab" type="button" role="tab" aria-selected="true"
        aria-controls="file-source-pane" data-markdown-mode="source">Source</button>
    </div>
    <p data-markdown-error role="status" hidden></p>
    <div id="file-preview-pane" data-markdown-preview role="tabpanel"
      aria-labelledby="file-preview-tab" hidden></div>
    <div id="file-source-pane" data-markdown-source-panel role="tabpanel"
      aria-labelledby="file-source-tab"><pre data-markdown-source></pre></div>
  </section>`;
  document.querySelector("[data-markdown-source]")!.textContent = source;
  return document.querySelector("[data-markdown-viewer]")!;
}

function boot(withLibraries = true): void {
  if (withLibraries) {
    browserScript(readFileSync(resolve(STATIC, "vendor/marked.umd.js"), "utf8"));
    browserScript(readFileSync(resolve(STATIC, "vendor/purify.min.js"), "utf8"));
  }
  browserScript(existsSync(RENDERER) ? readFileSync(RENDERER, "utf8") : "");
  document.dispatchEvent(new Event("DOMContentLoaded"));
}

function pane(): HTMLElement {
  return document.querySelector("[data-markdown-preview]")!;
}

function tab(mode: string): HTMLButtonElement {
  return document.querySelector(`[data-markdown-mode="${mode}"]`)!;
}

beforeEach(() => {
  document.body.innerHTML = "";
  window.history.replaceState(null, "", "/admin/vaults/v/files/notes/readme.md");
  delete previewWindow.marked;
  delete previewWindow.DOMPurify;
});

afterEach(() => {
  document.body.innerHTML = "";
  delete previewWindow.marked;
  delete previewWindow.DOMPurify;
  vi.restoreAllMocks();
});

describe("admin Markdown preview", () => {
  it("renders GFM by default and selects the preview tab", () => {
    viewer("# Heading\n\n**bold** and *emphasis*\n\n> Quote\n\n- One\n- Two\n\n| A | B |\n|---|---|\n| x | y |\n\n```js\nconst x = 1;\n```");
    boot();
    expect(pane().querySelector("h1")?.textContent).toBe("Heading");
    expect(pane().querySelector("strong")?.textContent).toBe("bold");
    expect(pane().querySelector("em")?.textContent).toBe("emphasis");
    expect(pane().querySelector("blockquote")?.textContent).toContain("Quote");
    expect(pane().querySelectorAll("li")).toHaveLength(2);
    expect(pane().querySelector("td")?.textContent).toBe("x");
    expect(pane().querySelector("pre code")?.textContent).toContain("const x = 1;");
    expect(pane().hidden).toBe(false);
    expect(document.querySelector<HTMLElement>("[data-markdown-source-panel]")!.hidden).toBe(true);
    expect(tab("preview").getAttribute("aria-selected")).toBe("true");
  });

  it("switches to the exact source and back without modifying it", () => {
    const source = "---\ntitle: Example\n---\n\n# 中文\n<script>alert(1)</script>\n";
    viewer(source);
    boot();
    tab("source").click();
    expect(pane().hidden).toBe(true);
    expect(document.querySelector<HTMLElement>("[data-markdown-source-panel]")!.hidden).toBe(false);
    expect(document.querySelector("[data-markdown-source]")!.textContent).toBe(source);
    tab("preview").click();
    expect(pane().hidden).toBe(false);
    expect(document.querySelector("[data-markdown-source]")!.textContent).toBe(source);
  });

  it("separates frontmatter into collapsed localized document properties", () => {
    viewer("---\ntitle: Example\ntags: [notes]\n---\n\n# Body");
    boot();
    const properties = pane().querySelector<HTMLDetailsElement>("details[data-markdown-properties]");
    expect(properties?.open).toBe(false);
    expect(properties?.querySelector("summary")?.textContent).toBe("Document properties");
    expect(properties?.querySelector("pre")?.textContent).toContain("title: Example");
    expect(pane().querySelector("h1")?.textContent).toBe("Body");
    expect(pane().querySelector("hr")).toBeNull();
  });

  it("handles BOM and CRLF frontmatter", () => {
    viewer("\uFEFF---\r\ntitle: 中文\r\n---\r\n\r\n# 正文");
    boot();
    expect(pane().querySelector("h1")?.textContent).toBe("正文");
    expect(pane().querySelector("[data-markdown-properties] pre")?.textContent).toContain("title: 中文");
  });

  it("keeps an unclosed frontmatter-looking prefix visible", () => {
    viewer("---\ntitle: unfinished\n\nBody");
    boot();
    expect(pane().textContent).toContain("unfinished");
    expect(pane().querySelector("[data-markdown-properties]")).toBeNull();
  });

  it("renders safe content while stripping active HTML and app styling attributes", () => {
    viewer(`# Safe heading
<script>alert(1)</script><style>body{display:none}</style>
<svg onload="alert(1)"><a href="javascript:alert(1)">svg</a></svg>
<math><mtext>math</mtext></math><iframe src="https://example.com"></iframe>
<form action="/admin/logout"><button>Submit</button></form>
<p id="admin-sidebar-toggle" name="location" class="sidebar" style="position:fixed" onclick="alert(1)">safe paragraph</p>
<a href="javascript:alert(1)">bad link</a><img src="x" onerror="alert(1)">`);
    boot();
    expect(pane().querySelector("h1")?.textContent).toBe("Safe heading");
    expect(pane().querySelector("script,style,svg,math,iframe,form,button,img")).toBeNull();
    for (const node of Array.from(pane().querySelectorAll("*"))) {
      expect(Array.from(node.attributes).some(attr => /^on/i.test(attr.name))).toBe(false);
      expect(node.hasAttribute("style") || node.hasAttribute("name") || node.hasAttribute("class")).toBe(false);
    }
    expect(pane().querySelector('a[href^="javascript:"]')).toBeNull();
    expect(pane().querySelector("#admin-sidebar-toggle")).toBeNull();
  });

  it("keeps task checkboxes read-only and removes other inputs", () => {
    viewer("- [x] Done\n- [ ] Todo\n\n<input type=text value=secret><input type=file>");
    boot();
    const inputs = pane().querySelectorAll<HTMLInputElement>("input");
    expect(inputs).toHaveLength(2);
    expect(Array.from(inputs).every(input => input.type === "checkbox" && input.disabled)).toBe(true);
    expect(inputs[0].checked).toBe(true);
  });

  it("shows Markdown images as explicit links without loading them", () => {
    viewer("![diagram](https://example.com/diagram.png)");
    boot();
    expect(pane().querySelector("img")).toBeNull();
    const link = pane().querySelector<HTMLAnchorElement>("a");
    expect(link?.href).toBe("https://example.com/diagram.png");
    expect(link?.textContent).toContain("Image");
    expect(link?.textContent).toContain("diagram");
    expect(link?.target).toBe("_blank");
    expect(link?.rel).toContain("noopener");
    expect(link?.rel).toContain("noreferrer");
  });

  it("allows normal external links and strips unsafe protocols", () => {
    viewer('[Site](https://example.com) [data](data:text/html,evil) [file](file:///tmp/a)');
    boot();
    const link = pane().querySelector<HTMLAnchorElement>("a");
    expect(link?.href).toBe("https://example.com/");
    expect(link?.target).toBe("_blank");
    expect(link?.rel).toBe("noopener noreferrer");
    expect(pane().querySelector('a[href^="data:"],a[href^="file:"]')).toBeNull();
  });

  it("preserves historical revision on relative vault links, respecting explicit revisions", () => {
    window.history.replaceState(null, "", "/admin/vaults/v/files/notes/readme.md?at=abc123");
    viewer("[Next](next.md) [Other](other.md?at=def456)");
    boot();
    const links = pane().querySelectorAll<HTMLAnchorElement>("a");
    expect(links).toHaveLength(2);
    expect(new URL(links[0].href).pathname).toBe("/admin/vaults/v/files/notes/next.md");
    expect(new URL(links[0].href).searchParams.get("at")).toBe("abc123");
    expect(new URL(links[1].href).searchParams.get("at")).toBe("def456");
  });

  it("scopes unique heading IDs and connects document fragments", () => {
    viewer("[Jump](#中文标题)\n\n## 中文标题\n\n## 中文标题");
    boot();
    const headings = pane().querySelectorAll("h2");
    expect(headings).toHaveLength(2);
    expect(headings[0].id).toMatch(/^md:/);
    expect(headings[1].id).not.toBe(headings[0].id);
    expect(decodeURIComponent(pane().querySelector("a")!.getAttribute("href")!)).toBe(`#${headings[0].id.slice(3)}`);
  });

  it("supports keyboard tab selection with focus and roving tabindex", () => {
    viewer("# Example");
    boot();
    tab("preview").focus();
    tab("preview").dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    expect(document.activeElement).toBe(tab("source"));
    expect(tab("source").tabIndex).toBe(0);
    expect(tab("preview").tabIndex).toBe(-1);
    expect(pane().hidden).toBe(true);
    tab("source").dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true }));
    expect(document.activeElement).toBe(tab("preview"));
    expect(pane().hidden).toBe(false);
  });

  it("connects duplicate headings and headings whose names collide with duplicate suffixes", () => {
    viewer("[Second](#heading-1) [Third](#heading-1-1)\n\n# Heading\n\n# Heading\n\n# Heading-1");
    boot();
    const headings = pane().querySelectorAll("h1");
    expect(headings).toHaveLength(3);
    const links = pane().querySelectorAll("a");
    expect(links[0].getAttribute("href")).toBe(`#${headings[1].id.slice(3)}`);
    expect(links[1].getAttribute("href")).toBe(`#${headings[2].id.slice(3)}`);
  });

  it("keeps public heading slugs separate from private DOM IDs", () => {
    window.history.replaceState(null, "", "/admin/vaults/v/files/notes/readme.md#md-heading");
    const scroll = vi.fn();
    const previous = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollIntoView");
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { configurable: true, value: scroll });
    try {
      viewer("[Prefixed](#md-heading)\n\n# Heading\n\n# md-heading");
      boot();
      const headings = pane().querySelectorAll("h1");
      expect(headings).toHaveLength(2);
      expect(document.getElementById("md-heading")).toBeNull();
      expect(scroll.mock.contexts[0]).toBe(headings[1]);
      expect(pane().querySelector("a")?.getAttribute("href")).toBe("#md-heading");
    } finally {
      if (previous) Object.defineProperty(HTMLElement.prototype, "scrollIntoView", previous);
      else Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
    }
  });

  it("supports repeated clicks on the same logical fragment", () => {
    viewer("[Jump](#heading)\n\n# Heading");
    boot();
    const heading = pane().querySelector("h1")!;
    const scroll = vi.fn();
    heading.scrollIntoView = scroll;
    const link = pane().querySelector("a")!;
    link.click();
    link.click();
    expect(window.location.hash).toBe("#heading");
    expect(scroll).toHaveBeenCalledTimes(2);
  });

  it("resolves initial file-link fragments after the preview has rendered", () => {
    window.history.replaceState(null, "", "/admin/vaults/v/files/notes/readme.md#heading");
    const scroll = vi.fn();
    const previous = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollIntoView");
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { configurable: true, value: scroll });
    try {
      viewer("# Heading");
      boot();
      const heading = pane().querySelector("h1")!;
      expect(heading.id).toBe("md:heading");
      expect(scroll).toHaveBeenCalledOnce();
      expect(scroll.mock.contexts[0]).toBe(heading);
    } finally {
      if (previous) Object.defineProperty(HTMLElement.prototype, "scrollIntoView", previous);
      else Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
    }
  });

  it("resolves same-file path fragments on hash changes", () => {
    const scroll = vi.fn();
    viewer("[Self](readme.md#heading)\n\n# Heading");
    boot();
    const heading = pane().querySelector("h1")!;
    heading.scrollIntoView = scroll;
    expect(pane().querySelector("a")?.hash).toBe("#heading");
    window.history.replaceState(null, "", "/admin/vaults/v/files/notes/readme.md#heading");
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(scroll).toHaveBeenCalledOnce();
  });

  it("rechecks the initial fragment after native load navigation before the next paint", () => {
    window.history.replaceState(null, "", "/admin/vaults/v/files/notes/readme.md#file-preview-tab");
    const frames: FrameRequestCallback[] = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation(callback => {
      frames.push(callback);
      return frames.length;
    });
    viewer("# file-preview-tab");
    boot();
    const heading = pane().querySelector("h1")!;
    expect(heading.textContent).toBe("file-preview-tab");
    const scroll = vi.fn();
    heading.scrollIntoView = scroll;
    window.dispatchEvent(new Event("load"));
    frames.forEach(callback => callback(0));
    expect(scroll).toHaveBeenCalledOnce();
  });

  it("falls back to readable source with a localized message if dependencies are missing", () => {
    viewer("# Example");
    boot(false);
    expect(document.querySelector<HTMLElement>("[data-markdown-source-panel]")!.hidden).toBe(false);
    const error = document.querySelector<HTMLElement>("[data-markdown-error]")!;
    expect(error.hidden).toBe(false);
    expect(error.textContent).toBe("Preview unavailable; read the source.");
  });

  it("falls back to source when parsing fails", () => {
    viewer("# Example");
    browserScript(readFileSync(resolve(STATIC, "vendor/marked.umd.js"), "utf8"));
    browserScript(readFileSync(resolve(STATIC, "vendor/purify.min.js"), "utf8"));
    previewWindow.marked = { ...previewWindow.marked!, parse: () => { throw new Error("parse failed"); } };
    boot(false);
    expect(pane().hidden).toBe(true);
    expect(document.querySelector<HTMLElement>("[data-markdown-source-panel]")!.hidden).toBe(false);
    expect(document.querySelector<HTMLElement>("[data-markdown-error]")!.hidden).toBe(false);
  });

  it("initializes once when DOM readiness is signalled twice", () => {
    viewer("# Once");
    boot();
    document.dispatchEvent(new Event("DOMContentLoaded"));
    expect(pane().querySelectorAll("h1")).toHaveLength(1);
    tab("source").click();
    expect(pane().hidden).toBe(true);
  });

  it("leaves non-Markdown pages untouched", () => {
    document.body.innerHTML = '<pre class="file-preview"># Plain text</pre>';
    boot();
    expect(document.body.innerHTML).toBe('<pre class="file-preview"># Plain text</pre>');
  });
});
