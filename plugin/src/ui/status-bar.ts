import type { Strings } from "../i18n";
import { isLoggedIn, type PKVSyncSettings } from "../settings";
import { statusText } from "./status";

export class SyncStatusBar {
  private readonly statusEl: HTMLElement;

  constructor(
    statusEl: HTMLElement,
    private readonly settings: () => PKVSyncSettings,
    private readonly text: () => Strings
  ) {
    this.statusEl = statusEl;
  }

  update(): void {
    const t = this.text();
    this.statusEl.setText(
      isLoggedIn(this.settings())
        ? statusText("connected", "", t)
        : statusText("not_configured", "", t)
    );
  }

  setText(text: string): void {
    this.statusEl.setText(text);
  }
}
