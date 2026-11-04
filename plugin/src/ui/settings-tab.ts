import {
  type App,
  Notice,
  Platform,
  PluginSettingTab,
  setIcon,
  type SettingDefinitionItem,
  type SettingDefinitionRender
} from "obsidian";
import { ApiError } from "../api/client";
import type { ConnectionStamp } from "../connection-context";
import { DataCommitError } from "../plugin-store";
import type {
  MeResponse,
  ServerConfigResponse,
  TokenView,
  VaultSettings,
  VaultSummary
} from "../api/types";
import { formatBytes } from "../format";
import { format, languageInReview } from "../i18n";
import type PKVSyncPlugin from "../main";
import {
  type PluginLanguage,
  type PluginThemeMode,
  type PluginUpdateSource
} from "../settings";
import { listConflictFiles } from "../sync/conflict-files";
import { DeleteVaultModal } from "./delete-vault-modal";
import { localStateErrorMessage } from "./local-state-error";
import {
  formatDetailedUnixSeconds,
  formatRelativeUnixSeconds,
  TIMEZONE_OPTIONS
} from "../time";
import { parseServerUrl } from "../url";
import { errorToMessage } from "../util";

type ButtonVariant = "primary" | "secondary" | "ghost";

// The config folder can be renamed, so the starter list follows
// Vault#configDir instead of assuming the default folder name.
function recommendedDotSyncGlobs(configDir: string): string[] {
  return [`${configDir}/themes/**`, `${configDir}/snippets/**`];
}

export class PKVSyncSettingTab extends PluginSettingTab {
  private cfg: ServerConfigResponse | null = null;
  private renderId = 0;
  private syncTimeExpanded = false;
  private conflictCount: number | null = null;
  private conflictCountAt = 0;
  private allowlistLoadGeneration = 0;

  constructor(
    app: App,
    private plugin: PKVSyncPlugin
  ) {
    super(app, plugin);
  }

  // Fallback for Obsidian < 1.13.0, which has no declarative settings API and
  // therefore calls display() instead of rendering getSettingDefinitions().
  display(): void {
    this.renderInto(this.containerEl);
  }

  getSettingDefinitions(): SettingDefinitionItem[] {
    const t = this.plugin.text();
    const panel: SettingDefinitionRender = {
      name: t.settingsTitle,
      desc: t.selfHostedVaultSync,
      aliases: this.settingsSearchAliases(),
      render: (setting) => {
        setting.settingEl.empty();
        setting.settingEl.addClass("pkv-settings-declarative-host");
        this.renderInto(setting.settingEl);
      }
    };
    return [
      {
        type: "group",
        heading: t.settingsTitle,
        items: [panel]
      }
    ];
  }

  private renderInto(host: HTMLElement): void {
    host.empty();
    host.addClass("pkv-sync-settings-host");
    host.toggleClass("is-mobile", this.isMobileLayout());
    host.toggleClass("is-phone", Platform.isPhone);
    host.toggleClass("is-light-override", this.plugin.settings.themeMode === "light");
    host.toggleClass("is-dark-override", this.plugin.settings.themeMode === "dark");

    const shell = host.createDiv({ cls: "pkv-sync-app" });
    const panel = shell.createDiv({ cls: "pkv-sync-panel" });
    const renderId = ++this.renderId;

    if (this.plugin.settings.token) {
      this.renderSynced(panel, renderId);
      return;
    }
    if (this.cfg) {
      this.renderLogin(panel);
      return;
    }
    this.renderConnection(panel);
  }

  private refresh(): void {
    // >=1.13 re-renders from getSettingDefinitions(); older versions have no
    // update(), so they keep the imperative path.
    const modern = this as Partial<{ update(): void }>;
    if (typeof modern.update === "function") modern.update();
    else this.display();
  }

  // Search terms for the declarative settings API: the tab is one custom panel,
  // so the individual sections are only discoverable through these aliases.
  private settingsSearchAliases(): string[] {
    const t = this.plugin.text();
    const aliases = [
      t.serverUrl,
      t.deploymentKey,
      t.deviceName,
      t.timezone,
      t.connection,
      t.language,
      t.autoLanguage,
      t.englishLanguage,
      t.zhCnLanguage,
      t.zhHantLanguage,
      t.japaneseLanguage,
      t.koreanLanguage,
      t.themeMode,
      t.themeAuto,
      t.themeLight,
      t.themeDark,
      t.account,
      t.username,
      t.password,
      t.inviteCodeOptional,
      t.login,
      t.register,
      t.logout,
      t.changeServer,
      t.tokens,
      t.vaults,
      t.createVault,
      t.vaultSyncAllowlist,
      t.enableHistoryUi,
      t.syncNowButton,
      t.conflictFiles,
      t.deleteConflictsButton,
      t.settingsUpdateSection,
      t.checkForUpdates,
      t.updateSource,
      t.updateSourceServer,
      t.updateSourceGitHub,
      t.updateCheckNow,
      t.updateNow
    ];
    return Array.from(new Set(aliases.filter((alias) => alias.trim().length > 0)));
  }

  private renderConnection(panel: HTMLElement): void {
    const t = this.plugin.text();
    this.renderHeader(panel, {
      detail: t.selfHostedVaultSync,
      tone: "muted"
    });
    this.renderLanguage(panel);
    this.renderThemeMode(panel);
    this.renderSectionLabel(panel, t.connection);

    const serverUrl = this.renderTextField(panel, t.serverUrl, {
      placeholder: "https://sync.example.com/k_xxx/",
      value: this.plugin.settings.serverUrl,
      onInput: (value) => {
        // Draft only: per-keystroke commits tore down the engine and
        // persisted half-typed values (R7R3-1). The change/blur event
        // (onCommit below) and Connect perform the real commit.
        this.plugin.stageConnectionDraft({ serverUrl: value.trim() });
      },
      onCommit: async () => this.plugin.saveSettings({ rebuild: false })
    });
    const deploymentKey = this.renderTextField(panel, t.deploymentKey, {
      placeholder: "k_xxx",
      value: this.plugin.settings.deploymentKey,
      onInput: (value) => {
        this.plugin.stageConnectionDraft({ deploymentKey: value.trim() });
      },
      onCommit: async () => this.plugin.saveSettings({ rebuild: false })
    });
    this.renderTextField(panel, t.deviceName, {
      value: this.plugin.settings.deviceName,
      onInput: (value) => {
        this.plugin.settings.deviceName = value.trim();
      },
      onCommit: async () => this.plugin.saveSettings({ rebuild: false })
    });
    this.renderSelectField(
      panel,
      t.timezone,
      this.plugin.settings.timezone,
      TIMEZONE_OPTIONS,
      async (value) => {
        this.plugin.settings.timezone = value;
        await this.plugin.saveSettings({ rebuild: false });
      }
    );

    this.renderUpdates(panel);
    this.renderButton(panel, t.connect, "primary", async () => {
      let stamp: ConnectionStamp | null = null;
      try {
        const parsed = parseServerUrl(serverUrl.value, deploymentKey.value);
        stamp = await this.plugin.setConnectionPatch({
          serverUrl: parsed.serverUrl,
          deploymentKey: parsed.deploymentKey
        });
        if (!stamp) return;
        // allowRebuild=false: resumeSyncForCurrentScope owns the single
        // rebuild, so an accepted config does not start two engines.
        const cfg = await this.plugin.refreshServerCapabilities(false, { throwOnFailure: true });
        if (!cfg || !this.plugin.isConnectionCurrent(stamp)) return;
        this.cfg = cfg;
        await this.plugin.resumeSyncForCurrentScope(stamp);
        if (!this.plugin.isConnectionCurrent(stamp)) return;
        new Notice(
          format(t.connectedToServer, { serverName: this.serverHost() })
        );
        this.refresh();
      } catch (error) {
        if (stamp && !this.plugin.isConnectionCurrent(stamp)) return;
        if (error instanceof DataCommitError) {
          // Failures that pause sync are reported inside the publication gate.
          if (error.state === "restored") new Notice(localStateErrorMessage(error, t));
          return;
        }
        if (error instanceof ApiError && error.code === "setup_required") {
          new Notice(t.setupRequiredNotice);
          return;
        }
        new Notice(errorToMessage(error));
      }
    });
    this.renderConflictCleanup(panel);
  }

  private renderLanguage(panel: HTMLElement): void {
    const t = this.plugin.text();
    const row = panel.createDiv({ cls: "pkv-sync-inline-field" });
    row.createDiv({ cls: "pkv-sync-label", text: t.language });
    const selectWrap = row.createDiv({ cls: "pkv-sync-select-wrap pkv-sync-language-select" });
    const select = selectWrap.createEl("select", {
      cls: "pkv-sync-input pkv-sync-select"
    });
    const options: Array<{ value: PluginLanguage; label: string }> = [
      { value: "auto", label: t.autoLanguage },
      { value: "en", label: t.englishLanguage },
      { value: "zh-CN", label: t.zhCnLanguage },
      { value: "zh-Hant", label: t.zhHantLanguage },
      {
        value: "ja",
        label: `${t.japaneseLanguage} ${t.needsReviewSuffix}`
      },
      {
        value: "ko",
        label: `${t.koreanLanguage} ${t.needsReviewSuffix}`
      }
    ];
    for (const option of options) {
      select.createEl("option", { value: option.value, text: option.label });
    }
    select.value = this.plugin.settings.language;
    select.addEventListener("change", () => {
      const nextLanguage = select.value as PluginLanguage;
      this.plugin.settings.language = nextLanguage;
      if (languageInReview(nextLanguage)) {
        new Notice(t.translationNeedsReview);
      }
      void this.plugin.saveSettings({ rebuild: false }).then(() => this.refresh());
    });
    if (languageInReview(this.plugin.settings.language)) {
      const help = row.createEl("a", {
        cls: "pkv-sync-help-link",
        text: t.helpTranslate,
        href: "https://github.com/cyberkurry/pkv-sync/issues"
      });
      help.setAttr("target", "_blank");
      help.setAttr("rel", "noopener");
    }
  }

  private renderLogin(panel: HTMLElement): void {
    const t = this.plugin.text();
    this.renderHeader(panel, {
      detail: format(t.connectedToServer, { serverName: this.serverHost() }),
      tone: "success",
      divider: true
    });
    this.renderButton(panel, t.changeServer, "ghost", () =>
      this.showConnectionSettings()
    ).addClass("pkv-sync-change-server");
    this.renderLanguage(panel);
    this.renderThemeMode(panel);
    this.renderSectionLabel(panel, t.account);

    const username = this.renderTextField(panel, t.username, {
      placeholder: t.usernamePlaceholder
    });
    const password = this.renderTextField(panel, t.password, {
      placeholder: "********",
      type: "password"
    });
    const inviteCode = this.renderTextField(panel, t.inviteCodeOptional, {
      placeholder: t.inviteCodePlaceholder
    });

    const row = panel.createDiv({ cls: "pkv-sync-button-row" });
    this.renderButton(row, t.login, "primary", () =>
      this.login(username.value.trim(), password.value)
    );
    this.renderButton(row, t.register, "secondary", () =>
      this.register(
        username.value.trim(),
        password.value,
        inviteCode.value.trim()
      )
    );
    this.renderUpdates(panel);
    this.renderConflictCleanup(panel);
  }

  private renderSynced(panel: HTMLElement, renderId: number): void {
    const t = this.plugin.text();
    const lastSync =
      formatRelativeUnixSeconds(this.plugin.settings.lastSyncSuccessAt, t) ||
      t.neverSynced;
    this.renderHeader(panel, {
      detail: format(t.syncedSummary, { time: lastSync }),
      tone: "success",
      expandedDetail: this.syncDetailTime()
    });
    this.renderLanguage(panel);
    this.renderThemeMode(panel);

    const body = panel.createDiv({ cls: "pkv-sync-synced-body" });
    body.createDiv({ cls: "pkv-sync-loading", text: t.loadingAccount });
    void this.renderAccountDetails(body, renderId);
  }

  private async login(username: string, password: string): Promise<void> {
    try {
      const stamp = await this.plugin.captureConnection();
      const response = await this.plugin
        .apiForConnection(stamp)
        .login(
          username,
          password,
          this.plugin.settings.deviceId,
          this.plugin.settings.deviceName
        );
      const accepted = await this.plugin.setConnectionPatch(
        {
          token: response.token,
          userId: response.user_id,
          username: response.username
        },
        stamp
      );
      if (!accepted) return;
      new Notice(this.plugin.text().loggedIn);
      await this.plugin.resumeSyncForCurrentScope(accepted);
      this.refresh();
    } catch (error) {
      new Notice(errorToMessage(error));
    }
  }

  private async register(
    username: string,
    password: string,
    inviteCode: string
  ): Promise<void> {
    try {
      const stamp = await this.plugin.captureConnection();
      const response = await this.plugin
        .apiForConnection(stamp)
        .register(
          username,
          password,
          this.plugin.settings.deviceId,
          this.plugin.settings.deviceName,
          inviteCode || undefined
        );
      const accepted = await this.plugin.setConnectionPatch(
        {
          token: response.token,
          userId: response.user_id,
          username: response.username
        },
        stamp
      );
      if (!accepted) return;
      new Notice(this.plugin.text().registeredAndLoggedIn);
      await this.plugin.resumeSyncForCurrentScope(accepted);
      this.refresh();
    } catch (error) {
      new Notice(errorToMessage(error));
    }
  }

  private async renderAccountDetails(
    body: HTMLElement,
    renderId: number
  ): Promise<void> {
    try {
      // R7R3-2: capture the connection BEFORE the requests and verify it
      // after — a server switch while the responses were in flight used to
      // render the OLD server's vaults and devices into the NEW
      // connection's panel.
      const stamp = await this.plugin.captureConnection();
      const client = this.plugin.apiForConnection(stamp);
      const [me, tokens] = await Promise.all([
        client.me(),
        client.tokens()
      ]);
      if (renderId !== this.renderId) return;
      if (!this.plugin.isConnectionCurrent(stamp)) return;
      body.empty();
      this.renderUserCard(body, me);
      this.renderHistorySetting(body);
      this.renderVaults(body, me.vaults);
      this.renderConflictCleanup(body);
      this.renderDevices(body, tokens);
      this.renderUpdates(body);
    } catch (error) {
      if (renderId !== this.renderId) return;
      body.empty();
      body.createDiv({
        text: errorToMessage(error),
        cls: "pkv-sync-error"
      });
      this.renderButton(body, this.plugin.text().logout, "secondary", () =>
        this.logout()
      ).addClass("pkv-sync-error-action");
    }
  }

  private renderUserCard(body: HTMLElement, me: MeResponse): void {
    const t = this.plugin.text();
    const card = body.createDiv({ cls: "pkv-sync-user-card" });
    card.createDiv({
      cls: "pkv-sync-avatar",
      text: this.initialFor(me.username)
    });
    const meta = card.createDiv({ cls: "pkv-sync-user-meta" });
    meta.createDiv({ cls: "pkv-sync-user-name", text: me.username });
    meta.createDiv({ cls: "pkv-sync-user-server", text: this.serverHost() });
    this.renderButton(card, t.logout, "ghost", () => this.logout());
  }

  private renderHistorySetting(body: HTMLElement): void {
    const t = this.plugin.text();
    const row = body.createDiv({ cls: "pkv-sync-checkbox-row" });
    const label = row.createEl("label", { cls: "pkv-sync-checkbox-label" });
    const input = label.createEl("input", { type: "checkbox" });
    input.checked = this.plugin.settings.enableHistoryUi;
    label.createSpan({ text: t.enableHistoryUi });
    input.addEventListener("change", () => {
      this.plugin.settings.enableHistoryUi = input.checked;
      void this.plugin.saveSettings({ rebuild: false });
    });
  }

  private renderVaults(body: HTMLElement, vaults: VaultSummary[]): void {
    const t = this.plugin.text();
    this.renderSectionLabel(body, t.vaults);

    for (const vault of vaults) {
      const selected = this.plugin.settings.selectedVaultId === vault.id;
      const row = body.createDiv({
        cls: `pkv-sync-vault-row${selected ? " is-active" : ""}`
      });
      row.createDiv({ cls: "pkv-sync-vault-dot" });
      const meta = row.createDiv({ cls: "pkv-sync-vault-meta" });
      meta.createDiv({ cls: "pkv-sync-vault-name", text: vault.name });
      meta.createDiv({
        cls: "pkv-sync-vault-summary",
        text: format(t.vaultSelectableSummary, {
          fileCount: vault.file_count,
          size: formatBytes(vault.size_bytes),
          selected: ""
        })
      });
      const actions = row.createDiv({ cls: "pkv-sync-vault-actions" });
      const button = this.renderButton(
        actions,
        selected ? t.selectedVaultButton : t.useVaultButton,
        selected ? "primary" : "ghost",
        async () => {
          const accepted = await this.plugin.setSelectedVault(
            vault.id,
            vault.name
          );
          if (!accepted) return;
          new Notice(format(t.selectedVaultNotice, { name: vault.name }));
          await this.plugin.resumeSyncForCurrentScope();
          this.refresh();
        }
      );
      button.disabled = selected;

      this.renderButton(actions, t.deleteVaultButton, "ghost", () => {
        new DeleteVaultModal(this.app, vault, t, () =>
          this.deleteVaultAndRefresh(vault)
        ).open();
      });
    }

    void this.renderVaultSyncAllowlist(body);
    this.renderCreateVault(body);
    this.renderButton(
      body,
      t.syncNowButton,
      "primary",
      () => this.plugin.syncNowManual(),
      "refresh-cw"
    ).addClass("pkv-sync-sync-now");
  }

  async renderVaultSyncAllowlist(body: HTMLElement): Promise<void> {
    const t = this.plugin.text();
    const vaultId = this.plugin.settings.selectedVaultId;
    if (!vaultId) return;
    // V4: track whether the CURRENT server value has arrived. Saves
    // before that would PUT the empty textarea as a full replacement and
    // wipe the allowlist. The generation counter also stops a late load
    // from overwriting what the user typed meanwhile.
    let allowlistLoaded = false;
    let loadFailed = false;
    // `?? 0` tolerates a missing instance field (prototype-only callers
    // in tests); the guard compares numbers.
    const previousGeneration = this.allowlistLoadGeneration ?? 0;
    const loadGeneration = previousGeneration + 1;
    this.allowlistLoadGeneration = loadGeneration;

    const section = body.createDiv({ cls: "pkv-sync-vault-settings" });
    section.createDiv({ cls: "pkv-sync-section-label", text: t.vaultSyncAllowlist });
    const field = section.createDiv({ cls: "pkv-sync-field" });
    field.createEl("label", { cls: "pkv-sync-label", text: t.vaultSyncAllowlist });
    const textarea = field.createEl("textarea", {
      cls: "pkv-sync-input pkv-sync-textarea",
      attr: { placeholder: t.vaultSyncAllowlistPlaceholder }
    });
    field.createDiv({ cls: "pkv-sync-field-hint", text: t.vaultSyncAllowlistHint });
    const actions = section.createDiv({
      cls: "pkv-sync-button-row pkv-sync-allowlist-actions"
    });

    this.renderButton(
      actions,
      t.vaultSyncAllowlistStarterButton,
      "secondary",
      () => {
        textarea.value = recommendedDotSyncGlobs(
          this.app.vault.configDir
        ).join("\n");
      }
    );
    const saveHandler = async (): Promise<void> => {
      // V4: a save before the current value finished loading (or after
      // the load failed) would PUT the empty textarea as a full
      // replacement and silently wipe the server-side allowlist.
      if (!allowlistLoaded) {
        if (loadFailed) {
          // The load already failed: the button now reads "retry" — the
          // constructive action is to reload the panel, not to save an
          // unpopulated textarea.
          this.refresh();
          return;
        }
        new Notice(t.vaultSyncAllowlistNotLoaded);
        return;
      }
      saveButton.disabled = true;
      try {
        // R7R3-2: fence the write to the connection it was intended for.
        const stamp = await this.plugin.captureConnection();
        await this.plugin.apiForConnection(stamp).putVaultSettings(vaultId, {
          extra_sync_globs: this.parseGlobTextarea(textarea.value)
        });
        if (!this.plugin.isConnectionCurrent(stamp)) return;
        new Notice(t.vaultSyncAllowlistSaved);
      } catch (error) {
        new Notice(`${t.vaultSyncAllowlistSaveFailed}: ${errorToMessage(error)}`);
      } finally {
        saveButton.disabled = false;
      }
    };
    const saveButton = this.renderButton(
      actions,
      t.vaultSyncAllowlistSaveButton,
      "primary",
      saveHandler
    );
    // Disabled until the current value arrives (V4).
    saveButton.disabled = true;

    try {
      const settings: VaultSettings = await this.plugin
        .api()
        .getVaultSettings(vaultId);
      if (loadGeneration !== this.allowlistLoadGeneration) return;
      textarea.value = this.formatGlobTextarea(settings.extra_sync_globs);
      allowlistLoaded = true;
      saveButton.disabled = false;
    } catch (error) {
      if (loadGeneration !== this.allowlistLoadGeneration) return;
      new Notice(`${t.vaultSyncAllowlistLoadFailed}: ${errorToMessage(error)}`);
      // Failed load: the button becomes an explicit retry instead of a
      // destructive save of an unpopulated textarea (V4).
      loadFailed = true;
      saveButton.disabled = false;
      saveButton.setText(t.vaultSyncAllowlistRetryButton);
    }
  }

  renderUpdates(body: HTMLElement): void {
    const t = this.plugin.text();
    this.renderSectionLabel(body, t.settingsUpdateSection);
    const card = body.createDiv({ cls: "pkv-sync-update-card" });
    card.createDiv({
      cls: "pkv-sync-update-title",
      text: format(t.currentVersion, {
        version: this.plugin.manifest?.version ?? "0.0.0"
      })
    });
    card.createDiv({
      cls: "pkv-sync-update-meta",
      text: format(t.lastUpdateCheck, {
        time:
          formatRelativeUnixSeconds(
            this.plugin.settings.lastUpdateCheckAt,
            t
          ) || t.neverSynced
      })
    });

    const toggleRow = card.createDiv({ cls: "pkv-sync-checkbox-row" });
    const label = toggleRow.createEl("label", { cls: "pkv-sync-checkbox-label" });
    const input = label.createEl("input", { type: "checkbox" });
    input.checked = this.plugin.settings.checkForUpdates;
    label.createSpan({ text: t.checkForUpdates });
    input.addEventListener("change", () => {
      this.plugin.settings.checkForUpdates = input.checked;
      void this.plugin.saveSettings({ rebuild: false }).then(() => {
        this.plugin.scheduleUpdateChecks();
      });
    });

    this.renderSelectField(
      card,
      t.updateSource,
      this.plugin.settings.updateSource,
      [
        { value: "server", label: t.updateSourceServer },
        { value: "github", label: t.updateSourceGitHub }
      ],
      async (value) => {
        this.plugin.settings.updateSource = value as PluginUpdateSource;
        await this.plugin.saveSettings({ rebuild: false });
        await this.plugin.checkForPluginUpdates(true);
        this.refresh();
      }
    );

    if (this.plugin.availableUpdate) {
      const banner = card.createDiv({ cls: "pkv-sync-update-banner" });
      banner.createDiv({
        cls: "pkv-sync-update-title",
        text: format(t.updateAvailable, {
          version: this.plugin.availableUpdate.version
        })
      });
      banner.createEl("a", {
        cls: "pkv-sync-help-link",
        text: t.updateReleaseNotes,
        attr: {
          href: this.plugin.availableUpdate.releaseNotesUrl,
          target: "_blank",
          rel: "noopener"
        }
      });
    }

    const actions = card.createDiv({ cls: "pkv-sync-button-row" });
    this.renderButton(actions, t.updateCheckNow, "secondary", async () => {
      await this.plugin.checkForPluginUpdates(true);
      this.refresh();
    });
    const updateButton = this.renderButton(actions, t.updateNow, "primary", async () => {
      if (!this.plugin.availableUpdate) return;
      updateButton.disabled = true;
      try {
        await this.plugin.applyPluginUpdate(this.plugin.availableUpdate);
      } catch {
        // applyPluginUpdate already surfaced the failure Notice and
        // rethrows; swallowing here keeps the button state recoverable
        // (V6) — the banner still shows the update, so retry must stay
        // possible.
      } finally {
        // V6: reset from the CURRENT availability — a failed apply keeps
        // the update available (retry), a successful one clears it.
        updateButton.disabled = !this.plugin.availableUpdate;
      }
      this.refresh();
    });
    updateButton.disabled = !this.plugin.availableUpdate;
  }

  async deleteVaultAndRefresh(vault: VaultSummary): Promise<void> {
    // R7R3-2: fence the delete to the connection the user was looking at;
    // a late click must not delete on a server the user already left.
    const stamp = await this.plugin.captureConnection();
    await this.plugin.apiForConnection(stamp).deleteVault(vault.id);
    if (!this.plugin.isConnectionCurrent(stamp)) return;
    if (this.plugin.settings.selectedVaultId === vault.id) {
      const accepted = await this.plugin.setSelectedVault("", "", stamp);
      if (!accepted) return;
      this.plugin.invalidateSyncEngine();
    }
    new Notice(
      format(this.plugin.text().deletedVaultNotice, { name: vault.name })
    );
    this.refresh();
  }

  private renderCreateVault(body: HTMLElement): void {
    const t = this.plugin.text();
    const row = body.createDiv({ cls: "pkv-sync-create-row" });
    const input = row.createEl("input", {
      cls: "pkv-sync-input",
      attr: { placeholder: t.newVaultNamePlaceholder }
    });
    this.renderButton(row, t.createVault, "primary", async () => {
      const vaultName = input.value.trim();
      if (!vaultName) {
        new Notice(t.vaultNameRequired);
        return;
      }
      try {
        // R7R3-2: fence the create to the connection it was issued on — a
        // late create must not build a vault on the OLD server and select
        // it on the new one.
        const stamp = await this.plugin.captureConnection();
        const vault = await this.plugin
          .apiForConnection(stamp)
          .createVault(vaultName);
        if (!this.plugin.isConnectionCurrent(stamp)) return;
        const accepted = await this.plugin.setSelectedVault(
          vault.id,
          vault.name,
          stamp
        );
        if (!accepted) return;
        new Notice(format(t.createdVaultNotice, { name: vault.name }));
        await this.plugin.resumeSyncForCurrentScope();
        this.refresh();
      } catch (error) {
        new Notice(`${t.createVaultFailed}: ${errorToMessage(error)}`);
      }
    });
  }

  private renderConflictCleanup(body: HTMLElement): void {
    const t = this.plugin.text();
    const now = Date.now();
    if (this.conflictCount === null || now - this.conflictCountAt > 5000) {
      this.conflictCount = listConflictFiles(this.app.vault).length;
      this.conflictCountAt = now;
    }
    const count = this.conflictCount;
    this.renderSectionLabel(body, t.conflictFiles);
    const row = body.createDiv({ cls: "pkv-sync-conflict-row" });
    const meta = row.createDiv({ cls: "pkv-sync-conflict-meta" });
    meta.createDiv({ cls: "pkv-sync-conflict-title", text: t.conflictFiles });
    meta.createDiv({
      cls: "pkv-sync-conflict-summary",
      text: format(t.conflictFilesSummary, { count })
    });
    const button = this.renderButton(
      row,
      t.deleteConflictsButton,
      "secondary",
      async () => {
        await this.plugin.deleteConflictFiles();
        this.conflictCount = null;
        this.refresh();
      }
    );
    button.disabled = count === 0;
  }

  showConnectionSettings(): void {
    this.cfg = null;
    this.refresh();
  }

  private renderDevices(body: HTMLElement, tokens: TokenView[]): void {
    const t = this.plugin.text();
    this.renderSectionLabel(body, t.tokens);
    const list = body.createDiv({ cls: "pkv-sync-device-list" });
    for (const token of tokens) {
      const item = list.createDiv({
        cls: `pkv-sync-device-card${token.current ? " is-current" : ""}`
      });
      item.createDiv({ cls: "pkv-sync-device-status" });
      const name = item.createDiv({
        cls: "pkv-sync-device-name",
        text: token.device_name
      });
      if (token.current) {
        name.createSpan({ cls: "pkv-sync-device-badge", text: t.currentDeviceSuffix });
      }
    }
  }

  private renderHeader(
    panel: HTMLElement,
    options: {
      detail: string;
      tone: "muted" | "success";
      divider?: boolean;
      expandedDetail?: string;
    }
  ): void {
    const header = panel.createDiv({ cls: "pkv-sync-header" });
    const logo = header.createDiv({ cls: "pkv-sync-logo" });
    setIcon(logo, "refresh-cw");
    const title = header.createDiv({ cls: "pkv-sync-title-block" });
    title.createDiv({ cls: "pkv-sync-title", text: this.plugin.text().settingsTitle });
    this.renderHeaderDetail(title, options);
    if (options.divider) panel.createDiv({ cls: "pkv-sync-divider" });
  }

  private renderHeaderDetail(
    parent: HTMLElement,
    options: {
      detail: string;
      tone: "muted" | "success";
      expandedDetail?: string;
    }
  ): void {
    const detail = parent.createDiv({
      cls: `pkv-sync-header-detail is-${options.tone}`
    });
    if (!options.expandedDetail) {
      detail.setText(options.detail);
      return;
    }

    const toggle = detail.createEl("button", {
      cls: "pkv-sync-time-toggle",
      attr: { "aria-expanded": String(this.syncTimeExpanded) }
    });
    toggle.createSpan({ text: options.detail });
    toggle.createSpan({
      cls: `pkv-sync-time-caret${this.syncTimeExpanded ? " is-open" : ""}`,
      attr: { "aria-hidden": "true" }
    });
    toggle.addEventListener("click", () => {
      this.syncTimeExpanded = !this.syncTimeExpanded;
      this.refresh();
    });

    if (this.syncTimeExpanded) {
      parent.createDiv({
        cls: "pkv-sync-time-detail",
        text: options.expandedDetail
      });
    }
  }

  private renderThemeMode(panel: HTMLElement): void {
    const t = this.plugin.text();
    const row = panel.createDiv({ cls: "pkv-sync-inline-field pkv-sync-theme-field" });
    row.createDiv({ cls: "pkv-sync-label", text: t.themeMode });
    const mode = this.plugin.settings.themeMode;
    const button = row.createEl("button", {
      cls: `pkv-sync-theme-button is-${mode}`,
      text: ""
    });
    const label = this.themeModeLabel(mode);
    button.setAttr("type", "button");
    button.setAttr("aria-label", `${t.themeMode}: ${label}`);
    button.setAttr("title", `${t.themeMode}: ${label}`);
    button.setAttr("data-theme-mode", mode);
    const icon = button.createSpan({ cls: "pkv-sync-theme-icon" });
    setIcon(icon, this.themeModeIcon(mode));
    button.createSpan({ cls: "pkv-sync-theme-label", text: label });
    button.addEventListener("click", () => {
      this.plugin.settings.themeMode = this.nextThemeMode(mode);
      void this.plugin.saveSettings({ rebuild: false }).then(() => this.refresh());
    });
  }

  private themeModeLabel(mode: PluginThemeMode): string {
    const t = this.plugin.text();
    if (mode === "light") return t.themeLight;
    if (mode === "dark") return t.themeDark;
    return t.themeAuto;
  }

  private themeModeIcon(mode: PluginThemeMode): string {
    if (mode === "light") return "sun";
    if (mode === "dark") return "moon";
    return "monitor";
  }

  private nextThemeMode(mode: PluginThemeMode): PluginThemeMode {
    if (mode === "auto") return "light";
    if (mode === "light") return "dark";
    return "auto";
  }

  private renderSectionLabel(parent: HTMLElement, text: string): void {
    parent.createDiv({ cls: "pkv-sync-section-label", text });
  }

  private renderTextField(
    parent: HTMLElement,
    label: string,
    options: {
      value?: string;
      placeholder?: string;
      type?: string;
      onInput?: (value: string) => void;
      onCommit?: () => Promise<void>;
    } = {}
  ): HTMLInputElement {
    const field = parent.createDiv({ cls: "pkv-sync-field" });
    field.createEl("label", { cls: "pkv-sync-label", text: label });
    const input = field.createEl("input", {
      cls: "pkv-sync-input",
      attr: { placeholder: options.placeholder ?? "" }
    });
    input.type = options.type ?? "text";
    input.value = options.value ?? "";
    input.addEventListener("input", () => options.onInput?.(input.value));
    input.addEventListener("change", () => {
      if (options.onCommit) void options.onCommit();
    });
    return input;
  }

  private renderSelectField(
    parent: HTMLElement,
    label: string,
    value: string,
    options: Array<{ value: string; label: string }>,
    onChange: (value: string) => Promise<void>
  ): HTMLSelectElement {
    const field = parent.createDiv({ cls: "pkv-sync-field" });
    field.createEl("label", { cls: "pkv-sync-label", text: label });
    const selectWrap = field.createDiv({ cls: "pkv-sync-select-wrap" });
    const select = selectWrap.createEl("select", {
      cls: "pkv-sync-input pkv-sync-select"
    });
    for (const option of options) {
      select.createEl("option", { value: option.value, text: option.label });
    }
    select.value = value;
    select.addEventListener("change", () => void onChange(select.value));
    return select;
  }

  private renderButton(
    parent: HTMLElement,
    text: string,
    variant: ButtonVariant,
    onClick: () => void | Promise<void>,
    icon?: string
  ): HTMLButtonElement {
    const button = parent.createEl("button", {
      cls: `pkv-sync-button is-${variant}`,
      text
    });
    if (icon) {
      button.empty();
      const iconEl = button.createSpan({ cls: "pkv-sync-button-icon" });
      setIcon(iconEl, icon);
      button.createSpan({ text });
    }
    button.addEventListener("click", () => void onClick());
    return button;
  }

  private parseGlobTextarea(value: string): string[] {
    return value
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line.length > 0);
  }

  private formatGlobTextarea(globs: string[]): string {
    return globs.join("\n");
  }

  private async logout(): Promise<void> {
    const stamp = await this.plugin.captureConnection();
    try {
      await this.plugin.apiForConnection(stamp).logout();
    } catch {
      // Token may already be invalid server-side.
    }
    const accepted = await this.plugin.setConnectionPatch(
      { token: "", username: "", userId: "" },
      stamp
    );
    if (!accepted) return;
    this.refresh();
  }

  private serverHost(): string {
    try {
      return new URL(this.plugin.settings.serverUrl).host;
    } catch {
      return (
        this.plugin.settings.serverUrl
          .replace(/^https?:\/\//i, "")
          .split("/")[0]
          ?.trim() || "server"
      );
    }
  }

  private initialFor(value: string): string {
    return value.trim().charAt(0).toUpperCase() || "P";
  }

  private isMobileLayout(): boolean {
    return (
      Platform.isMobile ||
      Platform.isMobileApp ||
      Platform.isPhone ||
      Platform.isTablet ||
      Platform.isAndroidApp ||
      Platform.isIosApp
    );
  }

  private syncDetailTime(): string {
    return formatDetailedUnixSeconds(
      this.plugin.settings.lastSyncSuccessAt,
      this.plugin.settings.timezone
    );
  }
}
