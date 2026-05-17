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
