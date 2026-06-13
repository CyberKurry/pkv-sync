import { Notice, Platform, Plugin, TFile } from "obsidian";
import { ApiClient } from "./api/client";
import { HistoryApi } from "./api/history-client";
import { SyncApi } from "./api/sync-client";
import type { CommitSummary, ServerCapabilities } from "./api/types";
import { generateDeviceId } from "./device-id";
import {
  readPluginSettings,
  readSyncIndex,
  syncScopeKey,
  writePluginSettingsWithoutAuth,
  writePluginSettingsPatch,
  writeSyncIndex
} from "./plugin-data";
import { AuthStore, authFromSettings, migrateAuth } from "./sync/auth-store";
import {
  DEFAULT_SETTINGS,
  historyUiAvailable,
  normalizeDebounceMs,
  type PKVSyncSettings,
  isLoggedIn
} from "./settings";
import { Debouncer } from "./sync/debounce";
import { SyncEngine } from "./sync/engine";
import {
  detectObsidianSync,
  migrateToPkv,
  scanVaultForMigration,
  type MigrationApi
} from "./sync/migrate-from-obsidian-sync";
import {
  deleteConflictFiles,
  findConflictPairsForPath,
  findConflictPairsForPathWithKinds,
  type ConflictPair
} from "./sync/conflict-files";
import { registerCommands } from "./commands";
import { SyncOrchestrator } from "./sync-orchestrator";
import type { LocalIndex } from "./sync/types";
import { ObsidianVaultAdapter } from "./sync/vault-adapter";
import { restoreFileToCommit } from "./sync/restore";
import { format, strings, type Strings } from "./i18n";
import { DiffModal } from "./ui/diff-modal";
import { HistoryModal, shortCommit } from "./ui/history-modal";
import { RestoreConfirmModal } from "./ui/restore-confirm";
import { RollbackConfirmModal } from "./ui/rollback-confirm-modal";
import { PKVSyncSettingTab } from "./ui/settings-tab";
import { SyncStatusModal } from "./ui/sync-modal";
import { addConflictResolveMenuItem } from "./ui/conflict-menu";
import { ConflictsListModal } from "./ui/conflicts-list-modal";
import { ConflictResolveModal } from "./ui/conflict-resolve-modal";
import { MigrateModal } from "./ui/migrate-modal";
import { SyncStatusBar } from "./ui/status-bar";
import { formatRelativeUnixSeconds, formatUnixSeconds } from "./time";
import { SerializedPluginDataStore } from "./plugin-store";
import {
  recoverPendingUpdate,
  UpdateCheckService,
  type PluginFileAdapter,
  type PluginUpdateStatus
} from "./services/update-check";
import { errorToMessage, extensionOf } from "./util";

export default class PKVSyncPlugin extends Plugin {
  settings: PKVSyncSettings = DEFAULT_SETTINGS;
  availableUpdate: PluginUpdateStatus | null = null;
  statusEl: SyncStatusBar | null = null;
  private client: ApiClient | null = null;
  private historyClient: HistoryApi | null = null;
  private updateServiceCache: {
    service: UpdateCheckService;
    adapter: unknown;
    configDir: string;
    currentVersion: string;
    pluginId: string;
    pluginDir?: string;
  } | null = null;
  engine: SyncEngine | null = null;
  pushDebouncer: Debouncer | null = null;
  pollTimer: number | null = null;
  fallbackTimer: number | null = null;
  updateDelayTimer: number | null = null;
  updateIntervalTimer: number | null = null;
  private serverCapabilities: ServerCapabilities | null = null;
  syncGeneration = 0;
  private dataStore = new SerializedPluginDataStore(
    () => this.loadData(),
    (data) => this.saveData(data)
  );
  private authStore = new AuthStore(
    (key) => this.app.loadLocalStorage(key),
    (key, data) => this.app.saveLocalStorage(key, data)
  );
  private orchestratorInstance: SyncOrchestrator | null = null;

  private get orchestrator(): SyncOrchestrator {
    if (!this.orchestratorInstance) {
      this.orchestratorInstance = new SyncOrchestrator(this);
    }
    return this.orchestratorInstance;
  }

  async onload(): Promise<void> {
    const t = this.text();
    await recoverPendingUpdate({
      adapter: this.pluginFileAdapter(),
      configDir: this.app.vault.configDir,
      pluginId: this.manifest.id || "pkv-sync",
      pluginDir: this.manifest.dir
    });
    const rawData = await this.loadData();
    const migration = migrateAuth(this.authStore, rawData);
    if (migration.strippedData !== null) {
      await this.saveData(migration.strippedData);
    }
    this.settings = readPluginSettings(migration.strippedData ?? rawData);
    const auth = this.authStore.load();
    if (auth) {
      this.settings.deviceId = auth.deviceId;
      this.settings.token = auth.token ?? "";
      this.settings.serverUrl = auth.serverUrl;
      this.settings.deploymentKey = auth.deploymentKey ?? "";
      this.settings.userId = auth.userId ?? "";
    }
    let shouldSaveSettings = false;
    if (!this.settings.deviceId) {
      this.settings.deviceId = generateDeviceId();
      shouldSaveSettings = true;
    }
    if (!this.settings.deviceName) {
      this.settings.deviceName = this.defaultDeviceName();
      shouldSaveSettings = true;
    }
    if (shouldSaveSettings) {
      await this.saveSettings({ rebuild: false });
    }
    void this.refreshServerCapabilities();
    this.statusEl = new SyncStatusBar(
      this.addStatusBarItem(),
      () => this.settings,
      () => this.text()
    );
    this.updateStatus();
    this.addSettingTab(new PKVSyncSettingTab(this.app, this));
    this.registerVaultWatchers();
    registerCommands(this, t);
    this.registerHistoryFileMenu();
    this.rebuildSyncEngine();
    this.scheduleUpdateChecks();
  }

  onunload(): void {
    this.orchestrator.dispose();
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
      writePluginSettingsWithoutAuth(data, this.settings)
    );
    this.persistAuth();
    void this.refreshServerCapabilities();
    this.updateStatus();
    if (options.rebuild !== false) this.rebuildSyncEngine();
  }

  private persistAuth(): void {
    this.authStore.save(authFromSettings(this.settings));
  }

  private async saveSettingsPatch(
    patch: Partial<PKVSyncSettings>,
    options: { rebuild?: boolean } = {}
  ): Promise<void> {
    Object.assign(this.settings, patch);
    await this.dataStore.update((data) => writePluginSettingsPatch(data, patch));
    this.updateStatus();
    if (options.rebuild !== false) this.rebuildSyncEngine();
  }

  scheduleUpdateChecks(): void {
    this.orchestrator.scheduleUpdateChecks();
  }

  async checkForPluginUpdates(showNotice = false): Promise<PluginUpdateStatus | null> {
    if (!this.settings.checkForUpdates && !showNotice) return null;
    try {
      const update = await this.updateService().checkOnce(
        this.settings.updateSource
      );
      this.availableUpdate = update;
      await this.saveSettingsPatch(
        { lastUpdateCheckAt: Math.floor(Date.now() / 1000) },
        { rebuild: false }
      );
      if (showNotice && !update) new Notice(this.text().updateUpToDate);
      if (showNotice && update) {
        new Notice(format(this.text().updateAvailable, { version: update.version }));
      }
      return update;
    } catch (error) {
      if (showNotice) {
        new Notice(
          format(this.text().updateFailed, {
            reason: errorToMessage(error)
          })
        );
      }
      return null;
    }
  }

  async applyPluginUpdate(update: PluginUpdateStatus): Promise<void> {
    try {
      await this.updateService().applyUpdate(update);
      this.availableUpdate = null;
      new Notice(format(this.text().updateSuccess, { version: update.version }));
    } catch (error) {
      const reason = errorToMessage(error);
      const message = /sha256/i.test(reason)
        ? this.text().updateSha256Mismatch
        : format(this.text().updateFailed, { reason });
      new Notice(message);
      throw error;
    }
  }

  async loadSyncIndex(scopeKey = syncScopeKey(this.settings)): Promise<LocalIndex> {
    return this.dataStore.read((data) => readSyncIndex(data, scopeKey));
  }

  async saveSyncIndex(
    index: LocalIndex,
    scopeKey = syncScopeKey(this.settings)
  ): Promise<void> {
    await this.dataStore.update((data) =>
      writePluginSettingsWithoutAuth(writeSyncIndex(data, scopeKey, index), this.settings)
    );
  }

  /**
   * Atomic read-modify-write of the sync index. The updater runs inside the
   * SerializedPluginDataStore.update transaction, so concurrent callers
   * cannot observe a stale load between each other's writes.
   */
  async updateSyncIndex(
    updater: (index: LocalIndex) => LocalIndex | Promise<LocalIndex>,
    scopeKey = syncScopeKey(this.settings)
  ): Promise<void> {
    await this.dataStore.update(async (data) => {
      const current = readSyncIndex(data, scopeKey);
      const next = await updater(current);
      return writePluginSettingsWithoutAuth(writeSyncIndex(data, scopeKey, next), this.settings);
    });
  }

  updateStatus(): void {
    this.statusEl?.update();
  }

  private defaultDeviceName(): string {
    const t = this.text();
    const hostname = this.desktopHostname();
    if (hostname) return hostname;
    const vaultName = this.app.vault.getName?.().trim();
    const prefix = vaultName || "Obsidian";
    if (Platform.isAndroidApp) {
      return `${prefix} - ${t.defaultAndroidDevice}`;
    }
    if (Platform.isIosApp) {
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

  async recordSyncSuccess(generation: number): Promise<void> {
    if (generation !== this.syncGeneration) return;
    await this.saveSettingsPatch(
      { lastSyncSuccessAt: Math.floor(Date.now() / 1000) },
      { rebuild: false }
    );
  }

  private makeClient(): ApiClient {
    return new ApiClient({
      serverUrl: this.settings.serverUrl,
      deploymentKey: this.settings.deploymentKey,
      token: this.settings.token,
      pluginVersion: this.manifest.version
    });
  }

  private updateService(): UpdateCheckService {
    const api = this.api();
    const adapter = this.app.vault.adapter;
    const configDir = this.app.vault.configDir;
    const currentVersion = this.manifest.version;
    const pluginId = this.manifest.id || "pkv-sync";
    const pluginDir = this.manifest.dir;
    const cached = this.updateServiceCache;
    if (
      cached &&
      cached.adapter === adapter &&
      cached.configDir === configDir &&
      cached.currentVersion === currentVersion &&
      cached.pluginId === pluginId &&
      cached.pluginDir === pluginDir
    ) {
      return cached.service;
    }
    const service = new UpdateCheckService({
      api,
      adapter: this.pluginFileAdapter(),
      configDir,
      currentVersion,
      pluginId,
      pluginDir
    });
    this.updateServiceCache = {
      service,
      adapter,
      configDir,
      currentVersion,
      pluginId,
      pluginDir
    };
    return service;
  }

  private pluginFileAdapter(): PluginFileAdapter {
    const adapter = this.app.vault.adapter;
    return {
      read: (path) => adapter.read(path),
      write: (path, data) => adapter.write(path, data),
      remove: (path) => adapter.remove(path),
      mkdir: (path) => adapter.mkdir(path)
    };
  }

  private historyApi(): HistoryApi {
    const api = this.api();
    if (!this.historyClient) this.historyClient = new HistoryApi(api);
    return this.historyClient;
  }

  private async refreshServerCapabilities(): Promise<void> {
    if (!this.settings.serverUrl || !this.settings.deploymentKey) {
      this.serverCapabilities = null;
      return;
    }
    try {
      const cfg = await this.api().config();
      this.serverCapabilities = cfg.capabilities ?? { history: true, diff: true };
      // Mirror server-controlled push debounce into local settings so the
      // engine actually honours runtime tuning (Plan J Critical fix).
      const debounceMs = normalizeDebounceMs(
        cfg.push_debounce_ms,
        this.settings.debounceMs
      );
      if (debounceMs !== this.settings.debounceMs) {
        await this.saveSettingsPatch({ debounceMs }, { rebuild: true });
      }
    } catch {
      this.serverCapabilities = null;
    }
  }

  private rebuildSyncEngine(): void {
    this.orchestrator.rebuild();
  }

  private registerVaultWatchers(): void {
    this.orchestrator.registerVaultWatchers();
  }

  private registerHistoryFileMenu(): void {
    this.registerEvent(
      this.app.workspace.on("file-menu", (menu, file) => {
        const t = this.text();
        addConflictResolveMenuItem(
          menu,
          file,
          this.app.vault,
          t,
          (selectedFile) => this.openConflictResolutionFor(selectedFile)
        );
        if (!this.historyEnabled() || !(file instanceof TFile)) return;
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

  openConflictsList(
    pairsProvider?: () => ConflictPair[] | Promise<ConflictPair[]>
  ): void {
    const openList = (): void => {
      new ConflictsListModal(
        this.app,
        this.text(),
        openList,
        pairsProvider
      ).open();
    };
    openList();
  }

  private openConflictResolutionFor(file: TFile): void {
    const pairsProvider = (): Promise<ConflictPair[]> =>
      findConflictPairsForPathWithKinds(this.app.vault, file.path);
    const pairs = findConflictPairsForPath(this.app.vault, file.path);
    if (pairs.length === 0) {
      new Notice(this.text().conflictsListEmpty);
      return;
    }
    if (pairs.length === 1) {
      void pairsProvider().then((pairsWithKinds) => {
        const pair = pairsWithKinds[0];
        if (!pair) {
          new Notice(this.text().conflictsListEmpty);
          return;
        }
        new ConflictResolveModal(
          this.app,
          pair,
          this.text(),
          () => undefined
        ).open();
      });
      return;
    }
    this.openConflictsList(pairsProvider);
  }

  historyEnabled(): boolean {
    return historyUiAvailable(this.settings, this.serverCapabilities);
  }

  private diffEnabled(): boolean {
    return this.historyEnabled() && (this.serverCapabilities?.diff ?? true);
  }

  async openHistoryForActive(): Promise<void> {
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
        historyRollbackToHere: t.historyRollbackToHere,
        historyMoreActions: t.historyMoreActions,
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
        ),
      onRollback: (entry) => this.confirmVaultRollback(entry)
    }).open();
  }

  async openVaultHistory(): Promise<void> {
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
      new Notice(errorToMessage(error));
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
      new Notice(errorToMessage(error));
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
      new Notice(errorToMessage(error));
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
      new Notice(errorToMessage(error));
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

  private confirmVaultRollback(entry: CommitSummary): void {
    const t = this.text();
    const vaultId = this.settings.selectedVaultId;
    const vaultName = this.settings.selectedVaultName;
    if (!vaultId || !vaultName) {
      new Notice(t.noticeSyncNotReady);
      return;
    }
    new RollbackConfirmModal(this.app, {
      vaultName,
      commit: shortCommit(entry.commit),
      labels: t,
      onConfirm: async (confirmName) => {
        await new SyncApi(this.api()).restoreVault(vaultId, entry.commit, confirmName);
        new Notice(t.rollbackSuccess);
        await this.engine?.syncNow();
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
    const ext = extensionOf(path);
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
    await this.orchestrator.syncNow();
  }

  async openMigrationModal(): Promise<void> {
    const t = this.text();
    if (!isLoggedIn(this.settings)) {
      new Notice(t.noticeNotConfigured);
      return;
    }
    try {
      const detection = await detectObsidianSync(this.app.vault);
      const scan = scanVaultForMigration(this.app.vault);
      const initialVaultName = this.app.vault.getName?.().trim() || t.vaultName;
      new MigrateModal(this.app, t, {
        detection,
        scan,
        initialVaultName,
        onStart: async (vaultName, onProgress) => {
          this.invalidateSyncEngine();
          const syncApi = new SyncApi(this.api());
          const api: MigrationApi = {
            createVault: (name) => this.api().createVault(name),
            state: (vaultId, headSince) => syncApi.state(vaultId, headSince),
            uploadCheck: (vaultId, hashes) => syncApi.uploadCheck(vaultId, hashes),
            uploadBlob: (vaultId, hash, bytes) =>
              syncApi.uploadBlob(vaultId, hash, bytes),
            push: (vaultId, ifMatch, changes, deviceName) =>
              syncApi.push(vaultId, ifMatch, changes, deviceName)
          };
          const result = await migrateToPkv({
            vault: this.app.vault,
            api,
            vaultName,
            deviceName: this.settings.deviceName,
            textExtensions: new Set(this.settings.textExtensions),
            onProgress
          });
          this.settings.selectedVaultId = result.vaultId;
          this.settings.selectedVaultName = result.vaultName;
          this.settings.lastSyncSuccessAt = Math.floor(Date.now() / 1000);
          await this.saveSettings({ rebuild: false });
          await this.saveSyncIndex(result.index);
          this.rebuildSyncEngine();
          new Notice(
            format(t.migrateCompleteNotice, {
              name: result.vaultName,
              count: result.pushedFiles
            })
          );
          return result;
        }
      }).open();
    } catch (error) {
      new Notice(errorToMessage(error));
    }
  }

  invalidateSyncEngine(): void {
    this.orchestrator.invalidate();
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
      new Notice(errorToMessage(error));
      return 0;
    }
  }
}
