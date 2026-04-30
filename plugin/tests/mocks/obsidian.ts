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
