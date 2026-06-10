import { type App, Notice, Platform, PluginSettingTab, setIcon } from "obsidian";
import { ApiError } from "../api/client";
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
  normalizeTextExtensions,
  type PluginLanguage,
  type PluginThemeMode,
  type PluginUpdateSource
} from "../settings";
import { listConflictFiles } from "../sync/conflict-files";
import { DeleteVaultModal } from "./delete-vault-modal";
import {
  formatDetailedUnixSeconds,
  formatRelativeUnixSeconds,
  TIMEZONE_OPTIONS
} from "../time";
import { parseServerUrl } from "../url";
import { errorToMessage } from "../util";

type ButtonVariant = "primary" | "secondary" | "ghost";

const RECOMMENDED_DOT_SYNC_GLOBS = [
  ".obsidian/themes/**",
  ".obsidian/snippets/**"
];

export class PKVSyncSettingTab extends PluginSettingTab {
  private cfg: ServerConfigResponse | null = null;
  private renderId = 0;
  private syncTimeExpanded = false;

  constructor(
    app: App,
    private plugin: PKVSyncPlugin
  ) {
    super(app, plugin);
  }

  display(): void {
    const { containerEl } = this;
    containerEl.empty();
    containerEl.addClass("pkv-sync-settings-host");
    containerEl.toggleClass("is-mobile", this.isMobileLayout());
    containerEl.toggleClass("is-phone", Platform.isPhone);
    containerEl.toggleClass("is-light-override", this.plugin.settings.themeMode === "light");
    containerEl.toggleClass("is-dark-override", this.plugin.settings.themeMode === "dark");

    const shell = containerEl.createDiv({ cls: "pkv-sync-app" });
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

  private renderConnection(panel: HTMLElement): void {
    const t = this.plugin.text();
    this.renderHeader(panel, {
      detail: "Self-hosted vault synchronization",
      tone: "muted"
    });
    this.renderLanguage(panel);
    this.renderThemeMode(panel);
    this.renderSectionLabel(panel, t.connection);

    const serverUrl = this.renderTextField(panel, t.serverUrl, {
      placeholder: "https://sync.example.com/k_xxx/",
      value: this.plugin.settings.serverUrl,
      onInput: (value) => {
        this.plugin.settings.serverUrl = value.trim();
      },
      onCommit: async () => this.plugin.saveSettings({ rebuild: false })
    });
    const deploymentKey = this.renderTextField(panel, t.deploymentKey, {
      placeholder: "k_xxx",
      value: this.plugin.settings.deploymentKey,
      onInput: (value) => {
        this.plugin.settings.deploymentKey = value.trim();
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
      try {
        const parsed = parseServerUrl(serverUrl.value, deploymentKey.value);
        this.plugin.settings.serverUrl = parsed.serverUrl;
        this.plugin.settings.deploymentKey = parsed.deploymentKey;
        await this.plugin.saveSettings({ rebuild: false });
        this.cfg = await this.plugin.api().config();
        this.plugin.settings.textExtensions = normalizeTextExtensions(
          this.cfg.supported_text_extensions
        );
        await this.plugin.saveSettings({ rebuild: false });
        new Notice(
          format(t.connectedToServer, { serverName: this.serverHost() })
        );
        this.display();
      } catch (error) {
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
      void this.plugin.saveSettings({ rebuild: false }).then(() => this.display());
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
      placeholder: "Enter username"
    });
    const password = this.renderTextField(panel, t.password, {
      placeholder: "********",
      type: "password"
    });
    const inviteCode = this.renderTextField(panel, `${t.inviteCode} (optional)`, {
      placeholder: "Enter invite code"
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
      formatRelativeUnixSeconds(this.plugin.settings.lastSyncSuccessAt) ||
      t.neverSynced;
    this.renderHeader(panel, {
      detail: `Synced • ${lastSync}`,
      tone: "success",
      expandedDetail: this.syncDetailTime()
    });
    this.renderLanguage(panel);
    this.renderThemeMode(panel);

    const body = panel.createDiv({ cls: "pkv-sync-synced-body" });
    body.createDiv({ cls: "pkv-sync-loading", text: "Loading account..." });
    void this.renderAccountDetails(body, renderId);
  }

  private async login(username: string, password: string): Promise<void> {
    try {
      const response = await this.plugin
        .api()
        .login(
          username,
          password,
          this.plugin.settings.deviceId,
          this.plugin.settings.deviceName
        );
      this.plugin.settings.token = response.token;
      this.plugin.settings.userId = response.user_id;
      this.plugin.settings.username = response.username;
      await this.plugin.saveSettings();
      new Notice(this.plugin.text().loggedIn);
      this.display();
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
      const response = await this.plugin
        .api()
        .register(
          username,
          password,
          this.plugin.settings.deviceId,
          this.plugin.settings.deviceName,
          inviteCode || undefined
        );
      this.plugin.settings.token = response.token;
      this.plugin.settings.userId = response.user_id;
      this.plugin.settings.username = response.username;
      await this.plugin.saveSettings();
      new Notice(this.plugin.text().registeredAndLoggedIn);
      this.display();
    } catch (error) {
      new Notice(errorToMessage(error));
    }
  }

  private async renderAccountDetails(
    body: HTMLElement,
    renderId: number
  ): Promise<void> {
    try {
      const [me, tokens] = await Promise.all([
        this.plugin.api().me(),
        this.plugin.api().tokens()
      ]);
      if (renderId !== this.renderId) return;
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
          this.plugin.settings.selectedVaultId = vault.id;
          this.plugin.settings.selectedVaultName = vault.name;
          await this.plugin.saveSettings();
          new Notice(format(t.selectedVaultNotice, { name: vault.name }));
          this.display();
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
    this.renderButton(body, t.syncNowButton, "primary", () =>
      this.plugin.syncNowManual()
    ).addClass("pkv-sync-sync-now");
  }

  async renderVaultSyncAllowlist(body: HTMLElement): Promise<void> {
    const t = this.plugin.text();
    const vaultId = this.plugin.settings.selectedVaultId;
    if (!vaultId) return;

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
        textarea.value = RECOMMENDED_DOT_SYNC_GLOBS.join("\n");
      }
    );
    const saveButton = this.renderButton(
      actions,
      t.vaultSyncAllowlistSaveButton,
      "primary",
      async () => {
        saveButton.disabled = true;
        try {
          await this.plugin.api().putVaultSettings(vaultId, {
            extra_sync_globs: this.parseGlobTextarea(textarea.value)
          });
          new Notice(t.vaultSyncAllowlistSaved);
        } catch (error) {
          new Notice(`${t.vaultSyncAllowlistSaveFailed}: ${errorToMessage(error)}`);
        } finally {
          saveButton.disabled = false;
        }
      }
    );

    try {
      const settings: VaultSettings = await this.plugin
        .api()
        .getVaultSettings(vaultId);
      textarea.value = this.formatGlobTextarea(settings.extra_sync_globs);
    } catch (error) {
      new Notice(`${t.vaultSyncAllowlistLoadFailed}: ${errorToMessage(error)}`);
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
          formatRelativeUnixSeconds(this.plugin.settings.lastUpdateCheckAt) ||
          t.neverSynced
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
        this.display();
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
      this.display();
    });
    const updateButton = this.renderButton(actions, t.updateNow, "primary", async () => {
      if (!this.plugin.availableUpdate) return;
      updateButton.disabled = true;
      await this.plugin.applyPluginUpdate(this.plugin.availableUpdate);
      this.display();
    });
    updateButton.disabled = !this.plugin.availableUpdate;
  }

  async deleteVaultAndRefresh(vault: VaultSummary): Promise<void> {
    await this.plugin.api().deleteVault(vault.id);
    if (this.plugin.settings.selectedVaultId === vault.id) {
      this.plugin.settings.selectedVaultId = "";
      this.plugin.settings.selectedVaultName = "";
      this.plugin.invalidateSyncEngine();
    }
    await this.plugin.saveSettings();
    new Notice(
      format(this.plugin.text().deletedVaultNotice, { name: vault.name })
    );
    this.display();
  }

  private renderCreateVault(body: HTMLElement): void {
    const t = this.plugin.text();
    const row = body.createDiv({ cls: "pkv-sync-create-row" });
    const input = row.createEl("input", {
      cls: "pkv-sync-input",
      attr: { placeholder: "New vault name..." }
    });
    this.renderButton(row, t.createVault, "primary", async () => {
      const vaultName = input.value.trim();
      if (!vaultName) {
        new Notice(t.vaultNameRequired);
        return;
      }
      try {
        const vault = await this.plugin.api().createVault(vaultName);
        this.plugin.settings.selectedVaultId = vault.id;
        this.plugin.settings.selectedVaultName = vault.name;
        await this.plugin.saveSettings();
        new Notice(format(t.createdVaultNotice, { name: vault.name }));
        this.display();
      } catch (error) {
        new Notice(`${t.createVaultFailed}: ${errorToMessage(error)}`);
      }
    });
  }

  private renderConflictCleanup(body: HTMLElement): void {
    const t = this.plugin.text();
    const count = listConflictFiles(this.app.vault).length;
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
        this.display();
      }
    );
    button.disabled = count === 0;
  }

  showConnectionSettings(): void {
    this.cfg = null;
    this.display();
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
      this.display();
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
      void this.plugin.saveSettings({ rebuild: false }).then(() => this.display());
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
    onClick: () => void | Promise<void>
  ): HTMLButtonElement {
    const button = parent.createEl("button", {
      cls: `pkv-sync-button is-${variant}`,
      text
    });
    if (text === this.plugin.text().syncNowButton) {
      button.empty();
      const icon = button.createSpan({ cls: "pkv-sync-button-icon" });
      setIcon(icon, "refresh-cw");
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
    try {
      await this.plugin.api().logout();
    } catch {
      // Token may already be invalid server-side.
    }
    this.plugin.settings.token = "";
    this.plugin.settings.username = "";
    this.plugin.settings.userId = "";
    await this.plugin.saveSettings();
    this.display();
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
