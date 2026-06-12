import { Notice } from "obsidian";
import { format, type Strings } from "./i18n";
import { isLoggedIn } from "./settings";
import { listConflictFiles } from "./sync/conflict-files";
import { formatUnixSeconds } from "./time";
import { statusText } from "./ui/status";
import { SyncStatusModal } from "./ui/sync-modal";
import { errorToMessage } from "./util";
import type PKVSyncPlugin from "./main";

export function registerCommands(plugin: PKVSyncPlugin, t: Strings): void {
  plugin.addCommand({
    id: "show-status",
    name: t.showStatusCommand,
    callback: () =>
      new Notice(
        isLoggedIn(plugin.settings)
          ? t.noticeConnected
          : t.noticeNotConfigured
      )
  });
  plugin.addCommand({
    id: "refresh-account",
    name: t.refreshAccountCommand,
    callback: async () => {
      try {
        const me = await plugin.api().me();
        plugin.settings.username = me.username;
        plugin.settings.userId = me.user_id;
        await plugin.saveSettings();
        new Notice(format(t.refreshedVaults, { count: me.vaults.length }));
      } catch (error) {
        new Notice(errorToMessage(error));
        plugin.statusEl?.setText(statusText("error", t.refreshFailed, t));
      }
    }
  });
  plugin.addCommand({
    id: "manual-sync",
    name: t.manualSyncCommand,
    callback: () => void plugin.syncNowManual()
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
