import type { Strings } from "../i18n";
import { isLoggedIn, type PKVSyncSettings } from "../settings";
import { statusText } from "./status";

export class SyncStatusBar {
  private readonly statusEl: HTMLElement;
  // R8-3: the engine's last setStatus() owns the bar while it lives.
  // update() used to unconditionally rewrite the text to
  // connected/not_configured, so any settings save (e.g. the update
  // check writing lastUpdateCheckAt) flipped an error/offline bar back
  // to "connected" until the next pass re-set it. Remember the engine
  // text; update() only fills in when there is nothing to remember.
  private engineText: string | null = null;

  constructor(
    statusEl: HTMLElement,
    private readonly settings: () => PKVSyncSettings,
    private readonly text: () => Strings
  ) {
    this.statusEl = statusEl;
  }

  update(): void {
    if (this.engineText !== null) return;
    const t = this.text();
    this.statusEl.setText(
      isLoggedIn(this.settings())
        ? statusText("connected", "", t)
        : statusText("not_configured", "", t)
    );
  }

  setText(text: string): void {
    this.engineText = text;
    this.statusEl.setText(text);
  }

  /** Engine teardown (logout, connection switch): forget its status. */
  clearEngineStatus(): void {
    this.engineText = null;
    this.update();
  }
}
