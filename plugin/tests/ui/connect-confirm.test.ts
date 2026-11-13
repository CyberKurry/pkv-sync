import { requestUrl } from "obsidian";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PKVSyncPlugin from "../../src/main";
import { strings } from "../../src/i18n";
import {
  applyConnectDeepLink,
  ConnectConfirmModal
} from "../../src/ui/connect-confirm";
import { createMigrationHarness } from "../helpers/migration-fixture";
import { notices } from "../mocks/obsidian";

/**
 * Minimal element stub following the settings-tab.test.ts MockElement
 * pattern: the component renders through Obsidian's createEl/createDiv
 * extensions, which a real DOM does not provide, so the harness — not the
 * component — supplies them.
 */
class StubElement {
  children: StubElement[] = [];
  private readonly clickListeners: Array<() => void> = [];
  text = "";
  cls = "";

  constructor(readonly tag: string) {}

  setText(next: string): void {
    this.text = next;
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

function openConfirmModal(
  server: string,
  deploymentKey: string,
  onConfirm: () => void | Promise<void>
): ConnectConfirmModal {
  const modal = new ConnectConfirmModal(
    {} as never,
    strings(),
    server,
    deploymentKey,
    onConfirm
  );
  modal.titleEl = new StubElement("h2") as never;
  modal.contentEl = new StubElement("div") as never;
  modal.onOpen();
  return modal;
}

describe("ConnectConfirmModal (3A)", () => {
  it("renders the target host and only fires on explicit confirm", async () => {
    const onConfirm = vi.fn();
    const modal = openConfirmModal("https://sync.example.com", "k_abc", onConfirm);

    expect(modal.contentEl.textContent).toContain("sync.example.com");
    const buttons = modal.contentEl.querySelectorAll("button");
    expect(buttons).toHaveLength(2);
    buttons[1].click(); // cancel
    expect(onConfirm).not.toHaveBeenCalled();
    buttons[0].click(); // confirm
    await vi.waitFor(() => expect(onConfirm).toHaveBeenCalledTimes(1));
  });

  it("falls back to the raw string when the server is not a parseable URL", () => {
    const modal = openConfirmModal("not a url", "k_abc", () => undefined);

    expect(modal.contentEl.textContent).toContain("not a url");
  });
});

describe("applyConnectDeepLink (3A)", () => {
  function response(data: unknown, status = 200) {
    return {
      status,
      headers: { "content-type": "application/json" },
      json: data,
      text: JSON.stringify(data),
      arrayBuffer: new ArrayBuffer(0)
    };
  }

  beforeEach(() => {
    vi.mocked(requestUrl).mockReset();
    notices.length = 0;
  });

  it("never writes connection state for an off-loopback http deep link", async () => {
    const h = createMigrationHarness();
    const setConnectionPatch = vi.spyOn(h.plugin, "setConnectionPatch");

    await applyConnectDeepLink(
      h.plugin,
      strings(),
      "http://attacker.example",
      "k_abc"
    );

    // The hostile link must die in parseServerUrl, before anything is
    // staged: no commit, no disk write, no network request.
    expect(setConnectionPatch).toHaveBeenCalledTimes(0);
    expect(h.saveCalls()).toBe(0);
    expect(h.plugin.settings.serverUrl).toBe("https://old.example.test");
    expect(requestUrl).not.toHaveBeenCalled();
    expect(notices).toEqual([
      "Server URL must use https unless it points to localhost"
    ]);
  });

  it("mirrors the settings-tab Connect flow for a benign link", async () => {
    const h = createMigrationHarness();
    h.plugin.apiForConnection = PKVSyncPlugin.prototype.apiForConnection;
    vi.mocked(requestUrl).mockResolvedValue(
      response({
        server_name: "Test",
        version: "1.7.1",
        registration: "disabled",
        max_file_size: 100_000_000,
        supported_text_extensions: h.plugin.settings.textExtensions,
        push_debounce_ms: h.plugin.settings.debounceMs
      })
    );

    await applyConnectDeepLink(
      h.plugin,
      strings("en"),
      "https://sync.example.com",
      "k_abc"
    );

    // Connection fields persist through the auth store (WithoutAuth data
    // writes); the harness proves the flow via the in-memory settings,
    // the data-write count and the resumed engine.
    expect(h.plugin.settings.serverUrl).toBe("https://sync.example.com");
    expect(h.plugin.settings.deploymentKey).toBe("k_abc");
    expect(h.saveCalls()).toBe(1);
    // The accepted switch invalidates the old identity's engine, then the
    // resume rebuilds exactly once — the settings-tab Connect sequence.
    expect(h.events).toEqual(["invalidate", "rebuild"]);
    expect(notices).toEqual(["Connected to sync.example.com"]);
  });
});
