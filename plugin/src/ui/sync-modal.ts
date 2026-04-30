import { App, Modal } from "obsidian";

export class SyncStatusModal extends Modal {
  constructor(
    app: App,
    private title: string,
    private text: string
  ) {
    super(app);
  }

  onOpen(): void {
    this.contentEl.empty();
    this.contentEl.createEl("h2", { text: this.title });
    this.contentEl.createEl("pre", { text: this.text });
  }
}
