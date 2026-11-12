import { type App, Modal } from "obsidian";
import { format, type Strings } from "../i18n";

/**
 * Escape hatch for the D3 empty-scan breaker (spec §3.4). The body text
 * deliberately warns that an empty scan usually means unavailable
 * storage — Syncthing's .stfolder incidents all happened AFTER users
 * manually overrode that guard.
 */
export class EmptyScanConfirmModal extends Modal {
  constructor(
    app: App,
    private readonly t: Strings,
    private readonly indexEntries: number,
    private readonly onAccept: () => void
  ) {
    super(app);
  }

  onOpen(): void {
    this.titleEl.setText(this.t.emptyScanConfirmTitle);
    this.contentEl.createEl("p", {
      text: format(this.t.emptyScanConfirmBody, { count: this.indexEntries })
    });
    // Plain DOM buttons (the plugin renders its own UI; no Setting
    // component). mod-warning is Obsidian's destructive button class.
    const row = this.contentEl.createDiv();
    const accept = row.createEl("button", {
      text: this.t.emptyScanConfirmAccept,
      cls: "mod-warning"
    });
    accept.addEventListener("click", () => {
      this.close();
      this.onAccept();
    });
    const cancel = row.createEl("button", { text: this.t.modalCancel });
    cancel.addEventListener("click", () => this.close());
  }
}
