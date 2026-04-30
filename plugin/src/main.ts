import { Notice, Plugin } from "obsidian";
import { ApiClient } from "./api/client";
import { SyncApi } from "./api/sync-client";
import {
  readPluginSettings,
  readSyncIndex,
  syncScopeKey,
  writePluginSettings,
  writeSyncIndex
} from "./plugin-data";
import {
  DEFAULT_SETTINGS,
  type PKVSyncSettings,
  isLoggedIn,
} from "./settings";
import { Debouncer } from "./sync/debounce";
import { SyncEngine } from "./sync/engine";
import type { LocalIndex } from "./sync/types";
import { ObsidianVaultAdapter, shouldSyncPath } from "./sync/vault-adapter";
import { format, strings, type Strings } from "./i18n";
import { PKVSyncSettingTab } from "./ui/settings-tab";
import { SyncStatusModal } from "./ui/sync-modal";
import { statusText } from "./ui/status";

export default class PKVSyncPlugin extends Plugin {
  settings: PKVSyncSettings = DEFAULT_SETTINGS;
  private statusEl: HTMLElement | null = null;
  private client: ApiClient | null = null;
  private engine: SyncEngine | null = null;
  private pushDebouncer: Debouncer | null = null;
  private pollTimer: number | null = null;
  private fallbackTimer: number | null = null;
  private syncGeneration = 0;

  async onload(): Promise<void> {
    const t = this.text();
    this.settings = readPluginSettings(await this.loadData());
    if (!this.settings.deviceName) {
      this.settings.deviceName = this.defaultDeviceName();
      await this.saveSettings();
    }
    this.client = this.makeClient();
    this.statusEl = this.addStatusBarItem();
    this.updateStatus();
    this.addSettingTab(new PKVSyncSettingTab(this.app, this));
    this.registerVaultWatchers();
    this.addCommand({
      id: "pkv-sync-show-status",
      name: t.showStatusCommand,
      callback: () =>
        new Notice(
          isLoggedIn(this.settings)
            ? t.noticeConnected
            : t.noticeNotConfigured
        )
    });
    this.addCommand({
      id: "pkv-sync-refresh-account",
      name: t.refreshAccountCommand,
      callback: async () => {
        try {
          const me = await this.api().me();
          this.settings.username = me.username;
          this.settings.userId = me.user_id;
          await this.saveSettings();
          new Notice(format(t.refreshedVaults, { count: me.vaults.length }));
        } catch (error) {
          new Notice(error instanceof Error ? error.message : String(error));
          this.statusEl?.setText(statusText("error", t.refreshFailed, t));
        }
      }
    });
    this.addCommand({
      id: "pkv-sync-manual-sync",
      name: t.manualSyncCommand,
      callback: () => void this.syncNowManual()
    });
    this.addCommand({
      id: "pkv-sync-view-status",
      name: t.viewSyncStatusCommand,
      callback: () => {
        const current = this.text();
        new SyncStatusModal(
          this.app,
          current.syncStatusTitle,
          format(current.syncStatusDetails, {
            server: this.settings.serverUrl,
            vault: this.settings.selectedVaultName || current.noneValue,
            user: this.settings.username || current.notLoggedInValue
          })
        ).open();
      }
    });
    this.addCommand({
      id: "pkv-sync-list-conflicts",
      name: t.listConflictsCommand,
      callback: () => {
        const current = this.text();
        const conflicts = this.app.vault
          .getFiles()
          .filter((file) => file.path.includes(".conflict-"));
        new SyncStatusModal(
          this.app,
          current.syncStatusTitle,
          conflicts.length
            ? conflicts.map((file) => file.path).join("\n")
            : current.noConflictFiles
        ).open();
      }
    });
    this.rebuildSyncEngine();
  }

  onunload(): void {
    this.pushDebouncer?.cancel();
    if (this.pollTimer !== null) window.clearInterval(this.pollTimer);
    if (this.fallbackTimer !== null) window.clearInterval(this.fallbackTimer);
    void this.engine?.flushOnUnload(1500);
    this.statusEl = null;
  }

  api(): ApiClient {
    if (!this.client) this.client = this.makeClient();
    this.client.update({
      serverUrl: this.settings.serverUrl,
      deploymentKey: this.settings.deploymentKey,
      token: this.settings.token
    });
    return this.client;
  }

  async saveSettings(): Promise<void> {
    await this.saveData(writePluginSettings(await this.loadData(), this.settings));
    this.client = this.makeClient();
    this.updateStatus();
    this.rebuildSyncEngine();
  }

  async loadSyncIndex(scopeKey = syncScopeKey(this.settings)): Promise<LocalIndex> {
    return readSyncIndex(await this.loadData(), scopeKey);
  }

  async saveSyncIndex(
    index: LocalIndex,
    scopeKey = syncScopeKey(this.settings)
  ): Promise<void> {
    const data = writeSyncIndex(await this.loadData(), scopeKey, index);
    await this.saveData(writePluginSettings(data, this.settings));
  }

  updateStatus(): void {
    const t = this.text();
    this.statusEl?.setText(
      isLoggedIn(this.settings)
        ? statusText("connected", "", t)
        : statusText("not_configured", "", t)
    );
  }

  private defaultDeviceName(): string {
    const t = this.text();
    const ua = navigator.userAgent.toLowerCase();
    if (ua.includes("android")) return t.defaultAndroidDevice;
    if (ua.includes("iphone") || ua.includes("ipad")) return t.defaultIosDevice;
    return t.defaultDesktopDevice;
  }

  private makeClient(): ApiClient {
    return new ApiClient({
      serverUrl: this.settings.serverUrl,
      deploymentKey: this.settings.deploymentKey,
      token: this.settings.token,
      pluginVersion: this.manifest.version
    });
  }

  private rebuildSyncEngine(): void {
    const generation = ++this.syncGeneration;
    this.pushDebouncer?.cancel();
    if (this.pollTimer !== null) {
      window.clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
    if (this.fallbackTimer !== null) {
      window.clearInterval(this.fallbackTimer);
      this.fallbackTimer = null;
    }
    this.engine = null;

    if (!isLoggedIn(this.settings) || !this.settings.selectedVaultId) return;

    const scopeKey = syncScopeKey(this.settings);
    const textExtensions = new Set(["md", "canvas", "base", "json", "txt", "css"]);
    this.engine = new SyncEngine({
      vaultId: this.settings.selectedVaultId,
      deviceName: this.settings.deviceName,
      textExtensions,
      vault: new ObsidianVaultAdapter(this.app.vault),
      api: new SyncApi(this.api()),
      index: {
        loadIndex: () => this.loadSyncIndex(scopeKey),
        saveIndex: async (index) => {
          if (generation !== this.syncGeneration) return;
          await this.saveSyncIndex(index, scopeKey);
        }
      },
      setStatus: (status, detail) =>
        generation === this.syncGeneration
          ? this.statusEl?.setText(statusText(status, detail, this.text()))
          : undefined
    });
    this.pushDebouncer = new Debouncer(this.settings.debounceMs, () => {
      void this.engine?.syncNow();
    });
    this.pollTimer = window.setInterval(() => {
      void this.engine?.syncNow();
    }, this.settings.pollIntervalSeconds * 1000);
    const fallbackMs = Math.max(
      30_000,
      Math.floor((this.settings.pollIntervalSeconds * 1000) / 2)
    );
    this.fallbackTimer = window.setInterval(() => {
      this.pushDebouncer?.trigger();
    }, fallbackMs);
    this.registerInterval(this.fallbackTimer);
    void this.engine.syncNow();
  }

  private registerVaultWatchers(): void {
    const scheduleForFile = (file: unknown) => {
      const path =
        typeof file === "object" && file !== null && "path" in file
          ? String((file as { path: unknown }).path)
          : "";
      if (path && shouldSyncPath(path)) this.pushDebouncer?.trigger();
    };

    this.registerEvent(this.app.vault.on("modify", scheduleForFile));
    this.registerEvent(this.app.vault.on("create", scheduleForFile));
    this.registerEvent(this.app.vault.on("delete", scheduleForFile));
    this.registerEvent(
      this.app.workspace.on("active-leaf-change", () => {
        this.pushDebouncer?.trigger();
      })
    );
    this.registerDomEvent(window, "blur", () => {
      void this.engine?.syncNow();
    });
  }

  text(): Strings {
    return strings(this.settings.language);
  }

  async syncNowManual(): Promise<void> {
    const t = this.text();
    if (!this.engine) {
      new Notice(t.noticeSyncNotReady);
      return;
    }
    try {
      await this.engine.syncNow();
      new Notice(t.noticeSyncComplete);
    } catch (error) {
      new Notice(error instanceof Error ? error.message : String(error));
    }
  }
}
