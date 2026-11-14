import { Notice } from "obsidian";
import { ApiError } from "../api/client";
import type { ServerConfigResponse } from "../api/types";
import type { ConnectionStamp } from "../connection-context";
import { format, type Strings } from "../i18n";
import { DataCommitError } from "../plugin-store";
import { parseServerUrl } from "../url";
import { errorToMessage } from "../util";
import { localStateErrorMessage } from "./local-state-error";

/**
 * Narrow slice of PKVSyncPlugin the connect flow needs. Keeping the
 * interface explicit lets the hostile-link regression run against the
 * real plugin prototype (migration harness) without booting onload().
 */
export interface ConnectFlowHost {
  setConnectionPatch(patch: {
    serverUrl: string;
    deploymentKey: string;
  }): Promise<ConnectionStamp | null>;
  refreshServerCapabilities(
    allowRebuild?: boolean,
    options?: { throwOnFailure?: boolean }
  ): Promise<ServerConfigResponse | null>;
  isConnectionCurrent(stamp: ConnectionStamp): boolean;
  resumeSyncForCurrentScope(expected?: ConnectionStamp): Promise<void>;
}

/**
 * The one connect flow, shared by the settings-tab Connect button and
 * the confirmed obsidian://pkv-connect deep link (which used to be two
 * byte-identical copies): commit, verify the connection is still
 * current, then resume, and report failures with the same localized
 * notices. parseServerUrl runs inside the try so both callers' raw
 * inputs go through it; it enforces https-off-loopback and shape rules,
 * so a hostile link dies before anything is staged.
 *
 * `onConnected` fires at the exact point the inline settings-tab flow
 * remembered the capabilities (between the currency guard and the
 * resume); the deep-link caller passes no hook.
 *
 * Returns true iff the connection completed and the success notice was
 * shown — the settings tab uses that to re-render only on the success
 * path, exactly as its inline flow did.
 */
export async function runConnectFlow(
  host: ConnectFlowHost,
  t: Strings,
  server: string,
  deploymentKey: string,
  onConnected?: (cfg: ServerConfigResponse) => void
): Promise<boolean> {
  let stamp: ConnectionStamp | null = null;
  try {
    const parsed = parseServerUrl(server, deploymentKey);
    stamp = await host.setConnectionPatch({
      serverUrl: parsed.serverUrl,
      deploymentKey: parsed.deploymentKey
    });
    if (!stamp) return false;
    // allowRebuild=false: resumeSyncForCurrentScope owns the single
    // rebuild, so an accepted config does not start two engines.
    const cfg = await host.refreshServerCapabilities(false, {
      throwOnFailure: true
    });
    if (!cfg || !host.isConnectionCurrent(stamp)) return false;
    onConnected?.(cfg);
    await host.resumeSyncForCurrentScope(stamp);
    if (!host.isConnectionCurrent(stamp)) return false;
    new Notice(
      format(t.connectedToServer, {
        serverName: new URL(parsed.serverUrl).host
      })
    );
    return true;
  } catch (error) {
    if (stamp && !host.isConnectionCurrent(stamp)) return false;
    if (error instanceof DataCommitError) {
      // Failures that pause sync are reported inside the publication gate.
      if (error.state === "restored") {
        new Notice(localStateErrorMessage(error, t));
      }
      return false;
    }
    if (error instanceof ApiError && error.code === "setup_required") {
      new Notice(t.setupRequiredNotice);
      return false;
    }
    new Notice(errorToMessage(error));
    return false;
  }
}
