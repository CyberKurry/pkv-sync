import { vi } from "vitest";

export const requestUrl = vi.fn();
export const notices: string[] = [];

export class Notice {
  constructor(public message: string) {
    notices.push(message);
  }
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
    createDiv: vi.fn()
  };

  constructor(public app: unknown, public plugin: unknown) {}

  display(): void {}
}

export function setIcon(): void {}
