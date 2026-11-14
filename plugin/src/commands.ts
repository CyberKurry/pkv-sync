import { Notice } from "obsidian";
import { format, type Strings } from "./i18n";
import { isLoggedIn } from "./settings";
import { listConflictFiles } from "./sync/conflict-files";
import { formatUnixSeconds } from "./time";
import { EmptyScanConfirmModal } from "./ui/empty-scan-confirm";
import { statusText } from "./ui/status";
import { SyncStatusModal } from "./ui/sync-modal";
import { errorToMessage } from "./util";
import type PKVSyncPlugin from "./main";

export function registerCommands(plugin: PKVSyncPlugin, t: Strings): void {
  plugin.addCommand({
    id: "show-status",
    name: t.showStatusCommand,
    callback: () => {
      // Fresh text() per invocation: the registration-time `t` would keep
      // speaking the language that was configured at startup.
      const current = plugin.text();
      new Notice(
        isLoggedIn(plugin.settings)
          ? current.noticeConnected
          : current.noticeNotConfigured
      );
    }
  });
  plugin.addCommand({
    id: "refresh-account",
    name: t.refreshAccountCommand,
    callback: async () => {
      const current = plugin.text();
      try {
        const stamp = await plugin.captureConnection();
        const me = await plugin.apiForConnection(stamp).me();
        const accepted = await plugin.setConnectionPatch(
          { username: me.username, userId: me.user_id },
          stamp
        );
        if (!accepted) return;
        await plugin.resumeSyncForCurrentScope(accepted);
        if (!plugin.isConnectionCurrent(accepted)) return;
        new Notice(format(current.refreshedVaults, { count: me.vaults.length }));
      } catch (error) {
        new Notice(errorToMessage(error));
        plugin.statusEl?.setText(statusText("error", current.refreshFailed, current));
      }
    }
  });
  plugin.addCommand({
    id: "manual-sync",
    name: t.manualSyncCommand,
    callback: () => void plugin.syncNowManual()
  });
  plugin.addCommand({
    id: "confirm-empty-vault-deletions",
    name: t.confirmEmptySyncCommand,
    callback: () => {
      const current = plugin.text();
      const engine = plugin.engine;
      if (!engine || !isLoggedIn(plugin.settings)) {
        new Notice(current.noticeNotConfigured);
        return;
      }
      new EmptyScanConfirmModal(
        plugin.app,
        current,
        engine.heldDeletionCount(),
        () => {
          // The modal can outlive the engine generation it was opened
          // against (rebuild/invalidate, M-10): re-resolve and never arm
          // a confirmation onto a different or missing engine. The accept
          // also fires long after the command ran, so its notice reads
          // text() again rather than the command-time snapshot.
          const atAccept = plugin.text();
          const active = plugin.engine;
          if (!active || active !== engine) {
            new Notice(atAccept.noticeNotConfigured);
            return;
          }
          active.allowEmptyScanDeletesOnce();
          void plugin.syncNowManual();
        }
      ).open();
    }
  });
  plugin.addCommand({
    id: "check-for-updates",
    name: t.checkForPluginUpdatesCommand,
    callback: () => void plugin.checkForPluginUpdates(true)
  });
  plugin.addCommand({
    id: "migrate-from-obsidian-sync",
    name: t.migrateCommand,
    callback: () => void plugin.openMigrationModal()
  });
  plugin.addCommand({
    id: "view-status",
    name: t.viewSyncStatusCommand,
    callback: () => {
      const current = plugin.text();
      new SyncStatusModal(
        plugin.app,
        current.syncStatusTitle,
        format(current.syncStatusDetails, {
          server: plugin.settings.serverUrl,
          vault: plugin.settings.selectedVaultName || current.noneValue,
          user: plugin.settings.username || current.notLoggedInValue,
          lastSync:
            formatUnixSeconds(
              plugin.settings.lastSyncSuccessAt,
              plugin.settings.timezone
            ) || current.neverSynced
        })
      ).open();
    }
  });
  plugin.addCommand({
    id: "list-conflicts",
    name: t.listConflictsCommand,
    callback: () => {
      const current = plugin.text();
      const conflicts = listConflictFiles(plugin.app.vault);
      new SyncStatusModal(
        plugin.app,
        current.syncStatusTitle,
        conflicts.length
          ? conflicts.map((file) => file.path).join("\n")
          : current.noConflictFiles
      ).open();
    }
  });
  plugin.addCommand({
    id: "delete-conflicts",
    name: t.deleteConflictsCommand,
    callback: () => void plugin.deleteConflictFiles()
  });
  plugin.addCommand({
    id: "resolve-conflicts",
    name: t.resolveConflictsCommand,
    callback: () => plugin.openConflictsList()
  });
  plugin.addCommand({
    id: "show-file-history",
    name: t.showFileHistoryCommand,
    checkCallback: (checking) => {
      if (!plugin.historyEnabled()) return false;
      if (!checking) void plugin.openHistoryForActive();
      return true;
    }
  });
  plugin.addCommand({
    id: "show-vault-history",
    name: t.showVaultHistoryCommand,
    checkCallback: (checking) => {
      if (!plugin.historyEnabled()) return false;
      if (!checking) void plugin.openVaultHistory();
      return true;
    }
  });
}
