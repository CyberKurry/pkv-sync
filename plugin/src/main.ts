import { Notice, Platform, Plugin, TFile } from "obsidian";
import { ApiClient } from "./api/client";
import { HistoryApi } from "./api/history-client";
import { SyncApi } from "./api/sync-client";
import type {
  CommitSummary,
  ServerCapabilities,
  ServerConfigResponse
} from "./api/types";
import {
  ConnectionContext,
  OperationStoppedError,
  type ConnectionIdentity,
  type ConnectionStamp,
  type RefreshTicket
} from "./connection-context";
import { generateDeviceId } from "./device-id";
import {
  readPluginSettings,
  readSyncIndex,
  syncScopeKey,
  writePluginSettingsWithoutAuth,
  writePluginSettingsPatch,
  writeSyncIndex
} from "./plugin-data";
import {
  AuthStore,
  authFromSettings,
  createElectronSafeStorage,
  migrateAuth
} from "./sync/auth-store";
import {
  DEFAULT_SETTINGS,
  historyUiAvailable,
  normalizeDebounceMs,
  normalizeTextExtensions,
  type PKVSyncSettings,
  isLoggedIn
} from "./settings";
import { Debouncer } from "./sync/debounce";
import { SyncEngine } from "./sync/engine";
import {
  detectObsidianSync,
  migrateToPkv,
  scanVaultForMigration,
  type MigrationProgress,
  type MigrationResult
} from "./sync/migrate-from-obsidian-sync";
import { writeMigrationCompletion } from "./sync/migration-commit";
import {
  deleteConflictFiles,
  findConflictPairsForPathWithKinds,
  listConflictFiles,
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
import { DeleteConflictsConfirmModal } from "./ui/delete-conflicts-confirm";
import { MigrateModal } from "./ui/migrate-modal";
import { localStateErrorMessage } from "./ui/local-state-error";
import { SyncStatusBar } from "./ui/status-bar";
import { formatRelativeUnixSeconds, formatUnixSeconds } from "./time";
import { isFetchableServerUrl } from "./url";
import { DataCommitError, SerializedPluginDataStore } from "./plugin-store";
import { LEGACY_PUSH_LIMITS, normalizePushLimits, type PushLimits } from "./sync/push-limits";
import {
  recoverPendingUpdate,
  UpdateCheckService,
  type PluginFileAdapter,
  type PluginUpdateStatus
} from "./services/update-check";
import { errorToMessage, isTextPath } from "./util";

type CompletionState = "old_safe" | "new_committed" | "disk_uncertain";

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
  serverPushLimits: Readonly<PushLimits> = LEGACY_PUSH_LIMITS;
  /**
   * Set when a local write could not be confirmed on disk. Every entry point
   * that would rebuild the engine checks it, so a later connection switch or
   * settings save cannot pretend the storage problem is gone.
   */
  persistenceRecoveryRequired = false;
  syncGeneration = 0;
  private connectionContextInstance: ConnectionContext | null = null;
  private dataStore = new SerializedPluginDataStore(
    () => this.loadData(),
    (data) => this.saveData(data)
  );
  private authStore = new AuthStore(
    (key) => this.app.loadLocalStorage(key),
    (key, data) => this.app.saveLocalStorage(key, data),
    createElectronSafeStorage()
  );
  private orchestratorInstance: SyncOrchestrator | null = null;

  private get orchestrator(): SyncOrchestrator {
    if (!this.orchestratorInstance) {
      this.orchestratorInstance = new SyncOrchestrator(this);
    }
    return this.orchestratorInstance;
  }

  private get connectionContext(): ConnectionContext {
    this.connectionContextInstance ??= new ConnectionContext(
      this.currentIdentity(),
      this.settings.selectedVaultId
    );
    return this.connectionContextInstance;
  }

  private currentIdentity(): ConnectionIdentity {
    return {
      serverUrl: this.settings.serverUrl,
      deploymentKey: this.settings.deploymentKey,
      token: this.settings.token,
      userId: this.settings.userId,
      username: this.settings.username
    };
  }

  /**
   * The committed connection identity — what the auth store must persist.
   *
   * Drafts (stageConnectionDraft) mutate this.settings in place, so
   * persisting auth straight from settings would write half-typed
   * connection values into the auth store — the ONLY persistence layer
   * for connection fields, and the source loadSettings() restores from on
   * the next launch (R8-1). The committed identity lives in the
   * connection context, which only setConnectionPatch replaces; every
   * auth persist therefore reads it instead of the drafted settings.
   */
  private committedIdentity(): ConnectionIdentity {
    const committed = this.connectionContextInstance?.snapshotLocked().connection;
    return committed ?? this.currentIdentity();
  }

  async onload(): Promise<void> {
    const t = this.text();
    await recoverPendingUpdate({
      adapter: this.pluginFileAdapter(),
      configDir: this.app.vault.configDir,
      pluginId: this.manifest.id || "pkv-sync",
      pluginDir: this.manifest.dir
    });
    const rawData: unknown = await this.loadData();
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
    // Identity and selection are captured ONCE from the fully merged
    // settings (data.json + auth store). Every later write is validated
    // against this baseline through the protected entries.
    this.connectionContextInstance = new ConnectionContext(
      this.currentIdentity(),
      this.settings.selectedVaultId
    );
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
    // R8-4: a settings-write failure (disk full, permissions) used to
    // escape as an unhandled rejection from every `void saveSettings()`
    // call site — memory settings already changed, disk not written, no
    // notice. Surface it through the localized notice so the user knows
    // the change did not persist.
    try {
      await this.connectionContext.runLocal(() => this.persistSettingsLocked());
    } catch (error) {
      new Notice(
        format(this.text().settingsSaveFailed, { reason: errorToMessage(error) })
      );
      throw error;
    }
    // W7: a rebuild-forbidden save (the migration path) must also forbid
    // the mirror's own rebuild.
    void this.refreshServerCapabilities(options.rebuild !== false);
    this.updateStatus();
    if (options.rebuild !== false) this.rebuildSyncEngine();
  }

  private async persistSettingsLocked(): Promise<void> {
    await this.dataStore.update((data) =>
      writePluginSettingsWithoutAuth(data, this.settings)
    );
    this.persistAuth();
  }

  currentPushLimits(): Readonly<PushLimits> {
    return this.serverPushLimits;
  }

  captureConnection(): Promise<ConnectionStamp> {
    return this.connectionContext.runLocal(() =>
      this.connectionContext.snapshotLocked()
    );
  }

  isConnectionCurrent(stamp: ConnectionStamp): boolean {
    return this.connectionContext.isConnectionCurrent(stamp);
  }

  apiForConnection(stamp: ConnectionStamp): ApiClient {
    return new ApiClient({
      serverUrl: stamp.connection.serverUrl,
      deploymentKey: stamp.connection.deploymentKey,
      token: stamp.connection.token,
      pluginVersion: this.manifest.version
    });
  }

  /**
   * Draft-stage a connection field edit: update the in-memory settings only.
   *
   * The settings tab fires this on every keystroke. The full
   * setConnectionPatch is a COMMIT: it replaces the connection identity
   * (bumping the epoch, which fences in-flight work from the old
   * connection), persists to disk and tears down the sync engine when the
   * identity changed. Doing that per keystroke meant one deleted character
   * left the vault with no engine, no timers, a persisted half-typed URL
   * and no recovery path until the next explicit connect (R7R3-1).
   *
   * Drafts are memory-only for the persisted identity: the engine keeps
   * running on the identity it was built with, and a drafted value only
   * becomes real through an explicit commit — the Connect button, which
   * calls setConnectionPatch (the single path that persists connection
   * fields, via persistAuth's committedIdentity). The field's
   * change/blur event persists non-connection settings only.
   */
  stageConnectionDraft(patch: Partial<ConnectionIdentity>): void {
    Object.assign(this.settings, patch);
  }

  async setConnectionPatch(
    patch: Partial<ConnectionIdentity>,
    expected?: ConnectionStamp
  ): Promise<ConnectionStamp | null> {
    const accepted = await this.connectionContext.runLocal(async () => {
      if (expected && !this.connectionContext.isConnectionCurrent(expected)) {
        return null;
      }
      const current = this.currentIdentity();
      const changed = this.connectionContext.replaceConnectionLocked({
        serverUrl: patch.serverUrl ?? current.serverUrl,
        deploymentKey: patch.deploymentKey ?? current.deploymentKey,
        token: patch.token ?? current.token,
        userId: patch.userId ?? current.userId,
        username: patch.username ?? current.username
      });
      Object.assign(this.settings, patch);
      if (changed) {
        // Derived state belongs to the connection it was fetched for.
        this.serverPushLimits = LEGACY_PUSH_LIMITS;
        this.serverCapabilities = null;
        this.invalidateSyncEngine();
      }
      await this.persistSettingsLocked();
      return this.connectionContext.snapshotLocked();
    });
    this.updateStatus();
    return accepted;
  }

  async setSelectedVault(
    vaultId: string,
    vaultName: string,
    expected?: ConnectionStamp
  ): Promise<boolean> {
    const accepted = await this.connectionContext.runLocal(async () => {
      if (expected && !this.connectionContext.isConnectionCurrent(expected)) {
        return false;
      }
      const changed = this.connectionContext.replaceSelectionLocked(vaultId);
      if (
        !changed &&
        this.settings.selectedVaultId === vaultId &&
        this.settings.selectedVaultName === vaultName
      ) {
        return true;
      }
      this.settings.selectedVaultId = vaultId;
      this.settings.selectedVaultName = vaultName;
      await this.persistSettingsLocked();
      return true;
    });
    this.updateStatus();
    return accepted;
  }

  async resumeSyncForCurrentScope(expected?: ConnectionStamp): Promise<void> {
    await this.connectionContext.runLocal(() => {
      if (expected && !this.connectionContext.isConnectionCurrent(expected)) return;
      if (this.persistenceRecoveryRequired || this.connectionContext.migrationPaused) {
        return;
      }
      this.rebuildSyncEngine();
    });
  }

  private persistAuth(): void {
    // R8-1: persist the COMMITTED identity, never the drafted settings —
    // a half-typed deployment key must not permanently replace k_xxx in
    // the store loadSettings() restores from.
    const committed = this.committedIdentity();
    this.authStore.save(
      authFromSettings({
        deviceId: this.settings.deviceId,
        serverUrl: committed.serverUrl,
        deploymentKey: committed.deploymentKey,
        token: committed.token,
        userId: committed.userId
      })
    );
  }

  private async saveSettingsPatch(
    patch: Partial<PKVSyncSettings>,
    options: { rebuild?: boolean; notice?: boolean } = {}
  ): Promise<void> {
    Object.assign(this.settings, patch);
    // R9-5: this is the second settings-write producer (lastSyncSuccessAt,
    // lastUpdateCheckAt). It used to call dataStore.update directly: a
    // disk write failure escaped into whichever caller happened to await
    // it - recordSyncSuccess turned a SUCCESSFUL sync pass into an error
    // status, checkForPluginUpdates reported "update check failed" - and
    // no notice ever named the real problem. Same contract as saveSettings
    // (R8-4): localized notice + rethrow, fenced by the connection lock.
    try {
      await this.connectionContext.runLocal(() =>
        this.dataStore.update((data) => writePluginSettingsPatch(data, patch))
      );
    } catch (error) {
      new Notice(
        format(this.text().settingsSaveFailed, { reason: errorToMessage(error) })
      );
      if (options.notice === false) {
        // Non-critical bookkeeping (timestamp remembers): report but do
        // not fail the caller's outcome - the sync pass itself succeeded.
        return;
      }
      throw error;
    }
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
    // R9-5: bookkeeping write — a failure must not rewrite a successful
    // pass into an error status (engine awaits this right after setting
    // "connected"). The notice still fires; the pass result stands.
    await this.saveSettingsPatch(
      { lastSyncSuccessAt: Math.floor(Date.now() / 1000) },
      { rebuild: false, notice: false }
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

  /**
   * Fetch GET /api/config and mirror the server-controlled knobs into
   * local settings.
   *
   * The response is accepted only if the connection it was requested for is
   * still current — a delayed answer from server A must never be applied
   * after the user switched to server B. Returns null when the answer was
   * dropped or a background request failed. Explicit connection attempts can
   * opt into receiving current-scope failures; stale failures are still dropped.
   *
   * `allowRebuild` is false on the migration path: the migration sets the
   * new vault id, then calls saveSettings({rebuild:false}) BEFORE saving
   * the migrated index. A mirror-triggered rebuild in that window would
   * start an engine against the new vault with the OLD (empty) index and
   * re-push the whole vault (W7).
   */
  async refreshServerCapabilities(
    allowRebuild = true,
    options: { throwOnFailure?: boolean } = {}
  ): Promise<ServerConfigResponse | null> {
    if (
      !isFetchableServerUrl(this.settings.serverUrl) ||
      !this.settings.deploymentKey
    ) {
      this.serverCapabilities = null;
      this.serverPushLimits = LEGACY_PUSH_LIMITS;
      return null;
    }
    const ticket = await this.connectionContext.captureRefresh();
    const client = this.apiForConnection(ticket);
    try {
      const cfg = await client.config();
      return await this.connectionContext.runLocal(async () => {
        if (!this.connectionContext.isRefreshCurrent(ticket)) return null;
        await this.applyServerConfigLocked(ticket, cfg, allowRebuild);
        return cfg;
      });
    } catch (error) {
      return this.connectionContext.runLocal(() => {
        if (!this.connectionContext.isRefreshCurrent(ticket)) return null;
        // Keep the current scope's last valid config; only a successful
        // answer may replace it.
        if (options.throwOnFailure) throw error;
        return null;
      });
    }
  }

  private async applyServerConfigLocked(
    ticket: RefreshTicket,
    cfg: ServerConfigResponse,
    allowRebuild: boolean
  ): Promise<void> {
    // Mirror server-controlled push debounce into local settings so the
    // engine actually honours runtime tuning (Plan J Critical fix).
    const debounceMs = normalizeDebounceMs(
      cfg.push_debounce_ms,
      this.settings.debounceMs
    );
    // S6: mirror the server's text-extension set on every config
    // fetch. A stale local set makes the client push text files to
    // paths the server has since reconfigured (or vice versa), and the
    // resulting wrong_file_kind 400 wedges the whole batch — every
    // sync pass re-sends the same rejected change forever.
    const serverExtensions = normalizeTextExtensions(
      cfg.supported_text_extensions
    );
    // W6: compare as sets — the server may reorder the array without
    // changing it, and a string comparison then triggered a needless
    // engine rebuild on every config fetch.
    const extensionsChanged =
      [...serverExtensions].sort().join("\u0000") !==
      [...this.settings.textExtensions].sort().join("\u0000");

    this.serverPushLimits = normalizePushLimits(cfg.push_limits);
    this.serverCapabilities = cfg.capabilities ?? { history: true, diff: true };

    if (debounceMs === this.settings.debounceMs && !extensionsChanged) return;

    const patch: Partial<PKVSyncSettings> = { debounceMs };
    if (extensionsChanged) patch.textExtensions = serverExtensions;

    try {
      await this.dataStore.commitWithRecovery(
        (raw) => writePluginSettingsPatch(raw, patch),
        {
          beforeSave: () => {
            if (!this.connectionContext.isRefreshCurrent(ticket)) {
              throw new OperationStoppedError("scope_changed");
            }
            return undefined;
          },
          publish: () => {
            Object.assign(this.settings, patch);
            if (allowRebuild && !this.connectionContext.migrationPaused) {
              this.rebuildSyncEngine();
            }
            return undefined;
          }
        }
      );
    } catch (error) {
      // The barrier must be raised BEFORE the gate is released: a queued
      // connection switch would otherwise rebuild an engine on top of disk
      // state we could not confirm, and an old request's failure would be
      // dismissed as stale afterwards.
      if (error instanceof DataCommitError && error.state !== "restored") {
        this.persistenceRecoveryRequired = true;
        this.invalidateSyncEngine();
        new Notice(localStateErrorMessage(error, this.text()));
      }
      throw error;
    }
  }

  private rebuildSyncEngine(): void {
    if (this.persistenceRecoveryRequired) return;
    // R8-3: a rebuild replaces the engine (and its generation guard);
    // the remembered engine status belongs to the old engine.
    this.statusEl?.clearEngineStatus();
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
    let openList = (): void => undefined;
    const onResolved = (): void => {
      this.pushDebouncer?.trigger();
      openList();
    };
    openList = (): void => {
      new ConflictsListModal(
        this.app,
        this.text(),
        onResolved,
        pairsProvider,
        this.settings.textExtensions
      ).open();
    };
    openList();
  }

  private openConflictResolutionFor(file: TFile): void {
    const pairsProvider = (): Promise<ConflictPair[]> =>
      findConflictPairsForPathWithKinds(this.app.vault, file.path);
    void pairsProvider()
      .then((pairs) => {
        if (pairs.length === 0) {
          new Notice(this.text().conflictsListEmpty);
          return;
        }
        if (pairs.length === 1) {
          const pair = pairs[0];
          new ConflictResolveModal(
            this.app,
            pair,
            this.text(),
            () => {
              this.pushDebouncer?.trigger();
            },
            this.settings.textExtensions
          ).open();
          return;
        }
        this.openConflictsList(pairsProvider);
      })
      .catch((error: unknown) => {
        // V7: a sidecar vanishing between listing and reading rejected
        // the whole read — the menu item silently did nothing.
        new Notice(
          `${this.text().conflictsListLoadFailed}: ${errorToMessage(error)}`
        );
      });
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
        historyUnknownDevice: t.historyUnknownDevice,
        loading: t.loading
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
        historyRetry: t.historyRetry,
        loading: t.loading
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
        (timestamp && formatRelativeUnixSeconds(timestamp, t)) ||
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
          app: this.app,
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
      const adapter = new ObsidianVaultAdapter(this.app);
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
    return !isTextPath(path, new Set(this.settings.textExtensions));
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
        onStart: async (vaultName, onProgress, isCancelled) => {
          const context = this.connectionContext;
          const ticket = await context.runLocal(() => {
            const began = context.beginMigrationLocked();
            this.invalidateSyncEngine();
            return began;
          });
          const capturedSettings = { ...this.settings };
          // The completion is decided by explicit state, never inferred from
          // "did we catch an error" — a committed record followed by a failed
          // engine rebuild is not a rollback.
          let completion: CompletionState = "old_safe";
          let lastProgress: MigrationProgress | null = null;
          let result: MigrationResult | null = null;
          let failure: unknown = null;
          let engineFailed = false;

          const assertActive = () => {
            context.assertMigration(ticket);
            if (ticket.phase === "transferring" && isCancelled?.()) {
              throw new OperationStoppedError("cancelled");
            }
          };
          const report = (progress: MigrationProgress) => {
            lastProgress = progress;
            onProgress(progress);
          };

          try {
            const client = this.apiForConnection(ticket);
            const syncApi = new SyncApi(client);
            result = await migrateToPkv({
              vault: this.app.vault,
              api: {
                createVault: (name) => client.createVault(name),
                state: (vaultId, headSince) => syncApi.state(vaultId, headSince),
                uploadCheck: (vaultId, hashes) =>
                  syncApi.uploadCheck(vaultId, hashes),
                uploadBlob: (vaultId, hash, bytes) =>
                  syncApi.uploadBlob(vaultId, hash, bytes),
                push: (vaultId, ifMatch, changes, deviceName) =>
                  syncApi.push(vaultId, ifMatch, changes, deviceName)
              },
              vaultName,
              deviceName: capturedSettings.deviceName,
              textExtensions: new Set(capturedSettings.textExtensions),
              onProgress: report,
              assertActive,
              pushLimits: this.currentPushLimits(),
              pushLabels: this.text()
            });
            assertActive();

            const targetScopeKey = syncScopeKey({
              ...capturedSettings,
              selectedVaultId: result.vaultId
            });
            const selection = {
              selectedVaultId: result.vaultId,
              selectedVaultName: result.vaultName,
              lastSyncSuccessAt: Math.floor(Date.now() / 1000)
            };
            const committing: MigrationProgress = {
              ...(lastProgress ?? {
                stage: "pushing",
                totalFiles: result.scannedFiles,
                processedFiles: result.pushedFiles,
                pushedFiles: result.pushedFiles,
                skippedCount: result.skippedCount,
                totalBytes: result.totalBytes,
                uploadedBlobs: 0,
                totalBlobs: 0,
                currentBatch: result.batches,
                totalBatches: result.batches
              }),
              stage: "committing"
            };
            const committedIndex = result.index;

            await context.runLocal(async () => {
              try {
                await this.dataStore.commitWithRecovery(
                  (raw) =>
                    writeMigrationCompletion(
                      raw,
                      selection,
                      targetScopeKey,
                      committedIndex
                    ),
                  {
                    beforeSave: () => {
                      // This synchronous slice is the cancellation deadline:
                      // no await may be added between the check and the write.
                      assertActive();
                      context.beginCommitLocked(ticket);
                      report(committing);
                      return undefined;
                    },
                    publish: () => {
                      Object.assign(this.settings, selection);
                      context.adoptMigrationSelectionLocked(
                        ticket,
                        selection.selectedVaultId
                      );
                      completion = "new_committed";
                      return undefined;
                    }
                  }
                );
              } catch (error) {
                if (
                  error instanceof DataCommitError &&
                  error.state !== "restored"
                ) {
                  completion = "disk_uncertain";
                  this.persistenceRecoveryRequired = true;
                  this.invalidateSyncEngine();
                }
                throw error;
              }
            });
          } catch (error) {
            failure = error;
          } finally {
            await context.runLocal(() => {
              const owned = context.finishMigrationLocked(ticket);
              if (!owned) return;
              if (completion === "disk_uncertain" || this.persistenceRecoveryRequired) {
                return;
              }
              try {
                this.rebuildSyncEngine();
              } catch {
                engineFailed = true;
              }
            });
          }

          if (failure) {
            if (failure instanceof DataCommitError) {
              new Notice(
                failure.state === "restored"
                  ? t.migrateSaveFailed
                  : t.migrateRecoveryFailed
              );
            } else if (
              failure instanceof OperationStoppedError &&
              failure.reason !== "cancelled"
            ) {
              // Never surface the machine reason ("scope_changed" /
              // "superseded") to the user.
              const stopped = new Error(t.migrateScopeChanged);
              new Notice(stopped.message);
              throw stopped;
            }
            throw failure;
          }
          if (!result) throw new Error("migration_result_missing");
          if (engineFailed) {
            new Notice(t.migrateEngineFailed);
            throw new Error(t.migrateEngineFailed);
          }
          if (!context.isCompletionCurrent(ticket)) {
            throw new Error(t.migrateScopeChanged);
          }
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
    // R8-3: the engine's remembered status dies with the engine.
    this.statusEl?.clearEngineStatus();
    this.orchestrator.invalidate();
  }

  async deleteConflictFiles(): Promise<number> {
    const t = this.text();
    try {
      const count = listConflictFiles(this.app.vault).length;
      if (count === 0) {
        new Notice(t.noConflictFiles);
        return 0;
      }
      return await new Promise<number>((resolve) => {
        new DeleteConflictsConfirmModal({
          app: this.app,
          count,
          labels: t,
          onConfirm: async () => {
            resolve(await this.trashConflictFiles());
          },
          onClose: () => resolve(0)
        }).open();
      });
    } catch (error) {
      new Notice(errorToMessage(error));
      return 0;
    }
  }

  private async trashConflictFiles(): Promise<number> {
    const t = this.text();
    try {
      const count = await deleteConflictFiles(this.app);
      new Notice(
        count ? format(t.deletedConflictFiles, { count }) : t.noConflictFiles
      );
      return count;
    } catch (error) {
      new Notice(errorToMessage(error));
      return 0;
    }
  }
}
