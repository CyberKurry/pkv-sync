import { Notice, type App, Modal } from "obsidian";
import { ApiError } from "../api/client";
import type { ConnectionStamp } from "../connection-context";
import { format, type Strings } from "../i18n";
import { DataCommitError } from "../plugin-store";
import { parseServerUrl } from "../url";
import { errorToMessage } from "../util";
import { localStateErrorMessage } from "./local-state-error";

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
 * Narrow slice of PKVSyncPlugin the deep-link connect flow needs. Keeping
 * the interface explicit lets the hostile-link regression run against the
 * real plugin prototype (migration harness) without booting onload().
 */
export interface ConnectDeepLinkHost {
  setConnectionPatch(patch: {
    serverUrl: string;
    deploymentKey: string;
  }): Promise<ConnectionStamp | null>;
  refreshServerCapabilities(
    allowRebuild?: boolean,
    options?: { throwOnFailure?: boolean }
  ): Promise<unknown>;
  isConnectionCurrent(stamp: ConnectionStamp): boolean;
  resumeSyncForCurrentScope(expected?: ConnectionStamp): Promise<void>;
}

/**
 * Part 3A: apply a confirmed obsidian://pkv-connect link. Mirrors the
 * settings-tab Connect button exactly — commit, verify the connection is
 * still current, then resume, and report failures with the same localized
 * notices — so the deep link gains no second code path. parseServerUrl
 * enforces https-off-loopback and shape rules; a hostile link dies there,
 * before anything is staged.
 */
export async function applyConnectDeepLink(
  plugin: ConnectDeepLinkHost,
  t: Strings,
  server: string,
  deploymentKey: string
): Promise<void> {
  let stamp: ConnectionStamp | null = null;
  try {
    const parsed = parseServerUrl(server, deploymentKey);
    stamp = await plugin.setConnectionPatch({
      serverUrl: parsed.serverUrl,
      deploymentKey: parsed.deploymentKey
    });
    if (!stamp) return;
    // allowRebuild=false: resumeSyncForCurrentScope owns the single
    // rebuild, so an accepted config does not start two engines.
    const cfg = await plugin.refreshServerCapabilities(false, {
      throwOnFailure: true
    });
    if (!cfg || !plugin.isConnectionCurrent(stamp)) return;
    await plugin.resumeSyncForCurrentScope(stamp);
    if (!plugin.isConnectionCurrent(stamp)) return;
    new Notice(
      format(t.connectedToServer, {
        serverName: new URL(parsed.serverUrl).host
      })
    );
  } catch (error) {
    if (stamp && !plugin.isConnectionCurrent(stamp)) return;
    if (error instanceof DataCommitError) {
      // Failures that pause sync are reported inside the publication gate.
      if (error.state === "restored") {
        new Notice(localStateErrorMessage(error, t));
      }
      return;
    }
    if (error instanceof ApiError && error.code === "setup_required") {
      new Notice(t.setupRequiredNotice);
      return;
    }
    new Notice(errorToMessage(error));
  }
}
