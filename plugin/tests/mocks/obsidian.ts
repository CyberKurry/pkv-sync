import { vi } from "vitest";

export const requestUrl = vi.fn();
export const notices: string[] = [];
export const Platform = {
  isMobile: false,
  isMobileApp: false,
  isPhone: false,
  isTablet: false,
  isAndroidApp: false,
  isIosApp: false,
  isDesktop: true,
  isDesktopApp: true
};

export class Notice {
  constructor(public message: string) {
    notices.push(message);
  }
}

export class Modal {
  contentEl = createMockElement();
  modalEl = createMockElement();

  constructor(public app: unknown) {}

  open(): void {
    this.onOpen();
  }

  close(): void {
    this.onClose();
  }

  onOpen(): void {}

  onClose(): void {}
}

export class Menu {
  items: MenuItem[] = [];
  separators = 0;
  shownAt: unknown = null;

  addItem(callback: (item: MenuItem) => void): this {
    const item = new MenuItem();
    callback(item);
    this.items.push(item);
    return this;
  }

  addSeparator(): this {
    this.separators += 1;
    return this;
  }

  showAtMouseEvent(event: unknown): void {
    this.shownAt = event;
  }
}

export class MenuItem {
  title = "";
  warning = false;
  callback: (() => void) | null = null;

  setTitle(title: string): this {
    this.title = title;
    return this;
  }

  setWarning(warning: boolean): this {
    this.warning = warning;
    return this;
  }

  onClick(callback: () => void): this {
    this.callback = callback;
    return this;
  }
}

export class Plugin {
  manifest = { version: "0.0.0" };
}

export class TFile {
  path = "";
}

export class TFolder {
  path = "";
  children: unknown[] = [];
}

export class PluginSettingTab {
  containerEl = {
    empty: vi.fn(),
    addClass: vi.fn(),
    removeClass: vi.fn(),
    toggleClass: vi.fn(),
    createDiv: vi.fn()
  };

  constructor(public app: unknown, public plugin: unknown) {}

  display(): void {}
}

export function setIcon(): void {}

function createMockElement(): any {
  return {
    empty: vi.fn(),
    addClass: vi.fn(),
    removeClass: vi.fn(),
    setText: vi.fn(),
    createDiv: vi.fn(() => createMockElement()),
    createEl: vi.fn(() => createMockElement()),
    createSpan: vi.fn(() => createMockElement()),
    appendChild: vi.fn(),
    addEventListener: vi.fn(),
    setAttr: vi.fn(),
    toggleClass: vi.fn(),
    style: {},
    dataset: {}
  };
}
