import { Notice, Platform, Plugin, TFile } from "obsidian";
import { ApiClient } from "./api/client";
import { HistoryApi } from "./api/history-client";
import { SyncApi } from "./api/sync-client";
import type { CommitSummary, ServerCapabilities } from "./api/types";
import {
  readPluginSettings,
  readSyncIndex,
  syncScopeKey,
  writePluginSettings,
  writeSyncIndex
} from "./plugin-data";
import {
  DEFAULT_SETTINGS,
  historyUiAvailable,
  type PKVSyncSettings,
  isLoggedIn
} from "./settings";
import { Debouncer } from "./sync/debounce";
import { SyncEngine } from "./sync/engine";
import {
  deleteConflictFiles,
  listConflictFiles
} from "./sync/conflict-files";
import type { LocalIndex } from "./sync/types";
import { ObsidianVaultAdapter, shouldSyncPath } from "./sync/vault-adapter";
import { restoreFileToCommit } from "./sync/restore";
import { format, strings, type Strings } from "./i18n";
import { DiffModal } from "./ui/diff-modal";
import { HistoryModal, shortCommit } from "./ui/history-modal";
import { RestoreConfirmModal } from "./ui/restore-confirm";
import { PKVSyncSettingTab } from "./ui/settings-tab";
import { SyncStatusModal } from "./ui/sync-modal";
import { ConflictsListModal } from "./ui/conflicts-list-modal";
import { statusText } from "./ui/status";
import { formatRelativeUnixSeconds, formatUnixSeconds } from "./time";
import { SerializedPluginDataStore } from "./plugin-store";

export default class PKVSyncPlugin extends Plugin {
  settings: PKVSyncSettings = DEFAULT_SETTINGS;
  private statusEl: HTMLElement | null = null;
  private client: ApiClient | null = null;
  private engine: SyncEngine | null = null;
  private pushDebouncer: Debouncer | null = null;
  private pollTimer: number | null = null;
  private fallbackTimer: number | null = null;
  private serverCapabilities: ServerCapabilities | null = null;
  private syncGeneration = 0;
  private dataStore = new SerializedPluginDataStore(
    () => this.loadData(),
    (data) => this.saveData(data)
  );

  async onload(): Promise<void> {
    const t = this.text();
    this.settings = readPluginSettings(await this.loadData());
    let shouldSaveSettings = false;
    if (!this.settings.deviceId) {
      this.settings.deviceId = this.generateDeviceId();
      shouldSaveSettings = true;
    }
    if (!this.settings.deviceName) {
      this.settings.deviceName = this.defaultDeviceName();
      shouldSaveSettings = true;
    }
    if (shouldSaveSettings) {
      await this.saveSettings({ rebuild: false });
    }
    this.client = this.makeClient();
    void this.refreshServerCapabilities();
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
            user: this.settings.username || current.notLoggedInValue,
            lastSync:
              formatUnixSeconds(
                this.settings.lastSyncSuccessAt,
                this.settings.timezone
              ) || current.neverSynced
          })
        ).open();
      }
    });
    this.addCommand({
      id: "pkv-sync-list-conflicts",
      name: t.listConflictsCommand,
      callback: () => {
        const current = this.text();
        const conflicts = listConflictFiles(this.app.vault);
        new SyncStatusModal(
          this.app,
          current.syncStatusTitle,
          conflicts.length
            ? conflicts.map((file) => file.path).join("\n")
            : current.noConflictFiles
        ).open();
      }
    });
    this.addCommand({
      id: "pkv-sync-delete-conflicts",
      name: t.deleteConflictsCommand,
      callback: () => void this.deleteConflictFiles()
    });
    this.addCommand({
      id: "pkv-sync-resolve-conflicts",
      name: t.resolveConflictsCommand,
      callback: () => {
        const openList = (): void => {
          new ConflictsListModal(this.app, this.text(), openList).open();
        };
        openList();
      }
    });
    this.addCommand({
      id: "pkv-sync-show-file-history",
      name: t.showFileHistoryCommand,
      checkCallback: (checking) => {
        if (!this.historyEnabled()) return false;
        if (!checking) void this.openHistoryForActive();
        return true;
      }
    });
    this.addCommand({
      id: "pkv-sync-show-vault-history",
      name: t.showVaultHistoryCommand,
      checkCallback: (checking) => {
        if (!this.historyEnabled()) return false;
        if (!checking) void this.openVaultHistory();
        return true;
      }
    });
    this.registerHistoryFileMenu();
    this.rebuildSyncEngine();
  }

  onunload(): void {
    this.pushDebouncer?.cancel();
    // Starting a new sync during Obsidian teardown can leave vault writes half-done.
    this.engine?.stopEventSubscription();
    if (this.pollTimer !== null) window.clearInterval(this.pollTimer);
    if (this.fallbackTimer !== null) window.clearInterval(this.fallbackTimer);
    this.syncGeneration++;
    this.engine = null;
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

  async saveSettings(options: { rebuild?: boolean } = {}): Promise<void> {
    await this.dataStore.update((data) =>
      writePluginSettings(data, this.settings)
    );
    this.client = this.makeClient();
    void this.refreshServerCapabilities();
    this.updateStatus();
    if (options.rebuild !== false) this.rebuildSyncEngine();
  }

  async loadSyncIndex(scopeKey = syncScopeKey(this.settings)): Promise<LocalIndex> {
    return this.dataStore.read((data) => readSyncIndex(data, scopeKey));
  }

  async saveSyncIndex(
    index: LocalIndex,
    scopeKey = syncScopeKey(this.settings)
  ): Promise<void> {
    await this.dataStore.update((data) =>
      writePluginSettings(writeSyncIndex(data, scopeKey, index), this.settings)
    );
  }

  /**
   * Atomic read-modify-write of the sync index. The updater runs inside the
   * SerializedPluginDataStore.update transaction, so concurrent callers
   * cannot observe a stale load between each other's writes (GLM5 H-5).
   */
  async updateSyncIndex(
    updater: (index: LocalIndex) => LocalIndex | Promise<LocalIndex>,
    scopeKey = syncScopeKey(this.settings)
  ): Promise<void> {
    await this.dataStore.update(async (data) => {
      const current = readSyncIndex(data, scopeKey);
      const next = await updater(current);
      return writePluginSettings(writeSyncIndex(data, scopeKey, next), this.settings);
    });
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
    const hostname = this.desktopHostname();
    if (hostname) return hostname;
    const vaultName = this.app.vault.getName?.().trim();
    const prefix = vaultName || "Obsidian";
    const ua = navigator.userAgent.toLowerCase();
    if (Platform.isAndroidApp || ua.includes("android")) {
      return `${prefix} - ${t.defaultAndroidDevice}`;
    }
    if (Platform.isIosApp || ua.includes("iphone") || ua.includes("ipad")) {
      return `${prefix} - ${t.defaultIosDevice}`;
    }
    return `${prefix} - ${t.defaultDesktopDevice}`;
  }

  private desktopHostname(): string | null {
    if (!Platform.isDesktopApp) return null;
    try {
      const nodeRequire = (window as unknown as {
        require?: (module: string) => { hostname?: () => string };
      }).require;
      const hostname = nodeRequire?.("os")?.hostname?.().trim();
      return hostname || null;
    } catch {
      return null;
    }
  }

  private generateDeviceId(): string {
    const random =
      typeof crypto !== "undefined" && "randomUUID" in crypto
        ? crypto.randomUUID()
        : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
    return `dev_${random}`;
  }

  private async recordSyncSuccess(generation: number): Promise<void> {
    if (generation !== this.syncGeneration) return;
    this.settings.lastSyncSuccessAt = Math.floor(Date.now() / 1000);
    await this.saveSettings({ rebuild: false });
  }

  private makeClient(): ApiClient {
    return new ApiClient({
      serverUrl: this.settings.serverUrl,
      deploymentKey: this.settings.deploymentKey,
      token: this.settings.token,
      pluginVersion: this.manifest.version
    });
  }

  private historyApi(): HistoryApi {
    return new HistoryApi(this.api());
  }

  private async refreshServerCapabilities(): Promise<void> {
    if (!this.settings.serverUrl || !this.settings.deploymentKey) {
      this.serverCapabilities = null;
      return;
    }
    try {
      const cfg = await this.api().config();
      this.serverCapabilities = cfg.capabilities ?? { history: true, diff: true };
      const globs = cfg.extra_exclude_globs ?? [];
      let settingsDirty = false;
      if (
        globs.length !== this.settings.extraExcludeGlobs.length ||
        !globs.every((g, i) => g === this.settings.extraExcludeGlobs[i])
      ) {
        this.settings.extraExcludeGlobs = globs;
        settingsDirty = true;
      }
      // Mirror server-controlled push debounce into local settings so the
      // engine actually honours runtime tuning (Plan J Critical fix).
      if (
        typeof cfg.push_debounce_ms === "number" &&
        Number.isFinite(cfg.push_debounce_ms) &&
        cfg.push_debounce_ms > 0 &&
        cfg.push_debounce_ms !== this.settings.debounceMs
      ) {
        this.settings.debounceMs = cfg.push_debounce_ms;
        settingsDirty = true;
      }
      if (settingsDirty) {
        await this.saveSettings({ rebuild: true });
      }
    } catch {
      this.serverCapabilities = null;
    }
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
    const textExtensions = new Set(this.settings.textExtensions);
    this.engine = new SyncEngine({
      vaultId: this.settings.selectedVaultId,
      deviceName: this.settings.deviceName,
      textExtensions,
      extraExcludeGlobs: this.settings.extraExcludeGlobs,
      vault: new ObsidianVaultAdapter(this.app.vault),
      api: new SyncApi(this.api()),
      index: {
        loadIndex: () => this.loadSyncIndex(scopeKey),
        saveIndex: async (index) => {
          if (generation !== this.syncGeneration) return;
          await this.saveSyncIndex(index, scopeKey);
        },
        updateIndex: async (updater) => {
          if (generation !== this.syncGeneration) return;
          await this.updateSyncIndex(updater, scopeKey);
        }
      },
      setStatus: (status, detail) =>
        generation === this.syncGeneration
          ? this.statusEl?.setText(statusText(status, detail, this.text()))
          : undefined,
      onSyncSuccess: () => this.recordSyncSuccess(generation),
      deviceId: this.settings.deviceId,
      serverUrl: this.settings.serverUrl,
      deploymentKey: this.settings.deploymentKey,
      token: this.settings.token,
    });
    this.engine.startEventSubscription();
    this.pushDebouncer = new Debouncer(this.settings.debounceMs, () => {
      void this.engine?.syncNow();
    });
    this.pollTimer = window.setInterval(() => {
      void this.engine?.syncNow();
    }, this.settings.pollIntervalSeconds * 1000);
    this.registerInterval(this.pollTimer);
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

  private registerHistoryFileMenu(): void {
    this.registerEvent(
      this.app.workspace.on("file-menu", (menu, file) => {
        if (!this.historyEnabled() || !(file instanceof TFile)) return;
        const t = this.text();
        menu.addItem((item) => {
          item
            .setTitle(t.fileHistoryMenu)
            .setIcon("history")
            .onClick(() => void this.openHistoryFor(file));
        });
        if (!this.diffEnabled()) return;
        menu.addItem((item) => {
          item
            .setTitle(t.diffWithPreviousMenu)
            .setIcon("git-compare")
            .onClick(() => void this.openDiffWithPrevious(file));
        });
      })
    );
  }

  private historyEnabled(): boolean {
    return historyUiAvailable(this.settings, this.serverCapabilities);
  }

  private diffEnabled(): boolean {
    return this.historyEnabled() && (this.serverCapabilities?.diff ?? true);
  }

  private async openHistoryForActive(): Promise<void> {
    const file = this.app.workspace.getActiveFile();
    if (!(file instanceof TFile)) {
      new Notice(this.text().historyDisabled);
      return;
    }
    await this.openHistoryFor(file);
  }

  private async openHistoryFor(file: TFile): Promise<void> {
    const t = this.text();
    if (!this.historyEnabled()) {
      new Notice(t.historyDisabled);
      return;
    }
    const diffAvailable = this.diffEnabled();
    new HistoryModal(this.app, {
      api: this.historyApi(),
      vaultId: this.settings.selectedVaultId,
      path: file.path,
      timezone: this.settings.timezone,
      labels: {
        historyTitle: t.historyTitle,
        historyEmpty: t.historyEmpty,
        historyRetry: t.historyRetry,
        historyViewDiffPrevious: t.historyViewDiffPrevious,
        historyViewDiffHead: t.historyViewDiffHead,
        historyViewContent: t.historyViewContent,
        historyRestoreVersion: t.historyRestoreVersion,
        historyUnknownDevice: t.historyUnknownDevice
      },
      onDiffPrevious: diffAvailable
        ? (entry) =>
            this.openDiffFor(
              file.path,
              entry.parent ?? undefined,
              entry.commit,
              entry.change_type !== "deleted"
            )
        : undefined,
      onDiffHead: diffAvailable
        ? (entry) => this.openDiffWithHead(file.path, entry)
        : undefined,
      onViewContent: (entry) => this.openHistoricalContent(file.path, entry),
      onRestore: (entry) =>
        this.confirmRestore(
          file.path,
          entry.commit,
          this.isBinaryPath(file.path),
          entry.timestamp
        )
    }).open();
  }

  private async openVaultHistory(): Promise<void> {
    const t = this.text();
    if (!this.historyEnabled()) {
      new Notice(t.historyDisabled);
      return;
    }
    try {
      const commits = await this.historyApi().commits(
        this.settings.selectedVaultId,
        50
      );
      const text = commits.length
        ? commits.map((entry) => this.commitLine(entry)).join("\n")
        : t.historyEmpty;
      new SyncStatusModal(this.app, t.showVaultHistoryCommand, text).open();
    } catch (error) {
      new Notice(error instanceof Error ? error.message : String(error));
    }
  }

  private async openDiffWithPrevious(file: TFile): Promise<void> {
    const t = this.text();
    if (!this.diffEnabled()) {
      new Notice(t.historyDisabled);
      return;
    }
    try {
      const [entry] = await this.historyApi().fileHistory(
        this.settings.selectedVaultId,
        file.path,
        1
      );
      if (!entry) {
        new Notice(t.historyEmpty);
        return;
      }
      await this.openDiffFor(
        file.path,
        entry.parent ?? undefined,
        entry.commit,
        entry.change_type !== "deleted"
      );
    } catch (error) {
      new Notice(error instanceof Error ? error.message : String(error));
    }
  }

  private async openDiffWithHead(
    path: string,
    entry: CommitSummary
  ): Promise<void> {
    const t = this.text();
    if (!this.diffEnabled()) {
      new Notice(t.historyDisabled);
      return;
    }
    try {
      const [head] = await this.historyApi().commits(
        this.settings.selectedVaultId,
        1
      );
      const to = head?.commit ?? entry.commit;
      await this.openDiffFor(path, entry.commit, to);
    } catch (error) {
      new Notice(error instanceof Error ? error.message : String(error));
    }
  }

  private async openHistoricalContent(
    path: string,
    entry: CommitSummary
  ): Promise<void> {
    const t = this.text();
    try {
      const file = await this.historyApi().readFileAt(
        this.settings.selectedVaultId,
        path,
        entry.commit
      );
      if (file.kind === "binary") {
        new Notice(t.diffBinary);
        return;
      }
      new SyncStatusModal(
        this.app,
        `${path} @ ${shortCommit(entry.commit)}`,
        file.text
      ).open();
    } catch (error) {
      new Notice(error instanceof Error ? error.message : String(error));
    }
  }

  private async openDiffFor(
    path: string,
    from: string | undefined,
    to: string,
    allowRestoreRight = true
  ): Promise<void> {
    const t = this.text();
    if (!this.diffEnabled()) {
      new Notice(t.historyDisabled);
      return;
    }
    new DiffModal(this.app, {
      api: this.historyApi(),
      vaultId: this.settings.selectedVaultId,
      path,
      from,
      to,
      timezone: this.settings.timezone,
      allowRestoreRight,
      labels: {
        diffTitle: t.diffTitle,
        diffBinary: t.diffBinary,
        diffTruncated: t.diffTruncated,
        diffFrom: t.diffFrom,
        diffTo: t.diffTo,
        diffPrevious: t.diffPrevious,
        diffRestoreLeft: t.diffRestoreLeft,
        diffRestoreRight: t.diffRestoreRight,
        historyRetry: t.historyRetry
      },
      onRestore: (commit, isBinary) => this.confirmRestore(path, commit, isBinary)
    }).open();
  }

  private async confirmRestore(
    path: string,
    commit: string,
    isBinary: boolean,
    timestamp?: number
  ): Promise<void> {
    const t = this.text();
    const hasUnsyncedLocalChanges = await this.hasUnsyncedLocalChanges(path);
    new RestoreConfirmModal({
      app: this.app,
      fileName: path.split("/").pop() || path,
      atCommitShort: shortCommit(commit),
      atTimeRelative:
        (timestamp && formatRelativeUnixSeconds(timestamp)) ||
        (timestamp && formatUnixSeconds(timestamp, this.settings.timezone)) ||
        shortCommit(commit),
      hasUnsyncedLocalChanges,
      labels: {
        restoreConfirmTitle: t.restoreConfirmTitle,
        restoreConfirmBody: t.restoreConfirmBody,
        restoreUnsyncedWarning: t.restoreUnsyncedWarning,
        restoreCancel: t.restoreCancel,
        restoreConfirm: t.restoreConfirm
      },
      onConfirm: async () => {
        const result = await restoreFileToCommit({
          vault: this.app.vault,
          api: this.historyApi(),
          vaultId: this.settings.selectedVaultId,
          path,
          atCommit: commit,
          isBinary
        });
        if (result.ok) {
          new Notice(format(t.restoreSuccess, { path }));
          this.pushDebouncer?.trigger();
          return;
        }
        const reason =
          result.reason === "deleted_at_commit"
            ? t.restoreDeletedAtCommit
            : result.detail ?? result.reason;
        new Notice(format(t.restoreFailed, { reason }));
      }
    }).open();
  }

  private async hasUnsyncedLocalChanges(path: string): Promise<boolean> {
    try {
      const index = await this.loadSyncIndex();
      const adapter = new ObsidianVaultAdapter(this.app.vault);
      const lastSyncedHash = index.files[path]?.lastSyncedHash;
      let snapshot;
      try {
        snapshot = await adapter.snapshot(path, new Set(this.settings.textExtensions));
      } catch {
        return Boolean(lastSyncedHash);
      }
      return !lastSyncedHash || snapshot.hash !== lastSyncedHash;
    } catch {
      return false;
    }
  }

  private isBinaryPath(path: string): boolean {
    const ext = path.includes(".") ? path.split(".").pop()?.toLowerCase() : "";
    return !ext || !this.settings.textExtensions.includes(ext);
  }

  private commitLine(entry: CommitSummary): string {
    const device = entry.author_device || this.text().historyUnknownDevice;
    const time = formatUnixSeconds(entry.timestamp, this.settings.timezone);
    return `${shortCommit(entry.commit)}  ${time}  ${device}  ${entry.message.split(/\r?\n/, 1)[0]}`;
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

  invalidateSyncEngine(): void {
    this.engine?.stopEventSubscription();
    this.pushDebouncer?.cancel();
    if (this.pollTimer !== null) {
      window.clearInterval(this.pollTimer);
      this.pollTimer = null;
    }
    if (this.fallbackTimer !== null) {
      window.clearInterval(this.fallbackTimer);
      this.fallbackTimer = null;
    }
    this.syncGeneration++;
    this.engine = null;
  }

  async deleteConflictFiles(): Promise<number> {
    const t = this.text();
    try {
      const count = await deleteConflictFiles(this.app.vault);
      new Notice(
        count
          ? format(t.deletedConflictFiles, { count })
          : t.noConflictFiles
      );
      return count;
    } catch (error) {
      new Notice(error instanceof Error ? error.message : String(error));
      return 0;
    }
  }
}
