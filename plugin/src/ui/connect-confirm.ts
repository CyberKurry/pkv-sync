import { type App, Modal } from "obsidian";
import { format, type Strings } from "../i18n";
import { runConnectFlow, type ConnectFlowHost } from "./connect-flow";

/** Part 3A (spec §6.3.2): obsidian://pkv-connect links can be triggered by
 * any web page — never connect silently. Shows the target host and only
 * hands the connection to onConfirm on explicit confirmation. */
export class ConnectConfirmModal extends Modal {
  constructor(
    app: App,
    private readonly t: Strings,
    private readonly server: string,
    private readonly onConfirm: () => void | Promise<void>
  ) {
    super(app);
  }

  onOpen(): void {
    this.titleEl.setText(this.t.connectConfirmTitle);
    let host = this.server;
    try {
      host = new URL(this.server).host;
    } catch {
      // keep the raw string visible for the user to eyeball
    }
    this.contentEl.createEl("p", {
      text: format(this.t.connectConfirmBody, { server: host })
    });
    const row = this.contentEl.createDiv();
    // Confirm-first with Obsidian's mod-cta is intentional: connecting is
    // the primary action here (the settings page's Connect button wears
    // the same emphasis), not a destructive step to be de-emphasized.
    const connect = row.createEl("button", {
      text: this.t.connect,
      cls: "mod-cta"
    });
    connect.addEventListener("click", () => {
      // close() only starts the closing animation; without this the
      // button stays clickable and a double click runs two connects.
      connect.disabled = true;
      this.close();
      void this.onConfirm();
    });
    const cancel = row.createEl("button", { text: this.t.modalCancel });
    cancel.addEventListener("click", () => this.close());
  }

  onClose(): void {
    this.contentEl.empty();
  }
}

/**
 * Part 3A: apply a confirmed obsidian://pkv-connect link. Thin wrapper
 * around the shared connect flow (runConnectFlow, also used by the
 * settings-tab Connect button) so the deep link gains no second code
 * path; the hostile-link regression suite targets this entry point.
 */
export async function applyConnectDeepLink(
  plugin: ConnectFlowHost,
  t: Strings,
  server: string,
  deploymentKey: string
): Promise<void> {
  await runConnectFlow(plugin, t, server, deploymentKey);
}
