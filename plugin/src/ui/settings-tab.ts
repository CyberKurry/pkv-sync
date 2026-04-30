import { type App, Notice, PluginSettingTab, Setting } from "obsidian";
import { ApiError } from "../api/client";
import type { ServerConfigResponse } from "../api/types";
import { format } from "../i18n";
import type PKVSyncPlugin from "../main";
import type { PluginLanguage } from "../settings";
import { parseServerUrl } from "../url";

export class PKVSyncSettingTab extends PluginSettingTab {
  private cfg: ServerConfigResponse | null = null;

  constructor(
    app: App,
    private plugin: PKVSyncPlugin
  ) {
    super(app, plugin);
  }

  display(): void {
    const { containerEl } = this;
    const t = this.plugin.text();
    containerEl.empty();
    containerEl.createEl("h2", { text: t.settingsTitle });
    this.renderLanguage(containerEl);
    this.renderConnection(containerEl);
    this.renderAccount(containerEl);
  }

  private renderLanguage(containerEl: HTMLElement): void {
    const t = this.plugin.text();
    new Setting(containerEl).setName(t.language).addDropdown((dropdown) =>
      dropdown
        .addOption("auto", t.autoLanguage)
        .addOption("en", t.englishLanguage)
        .addOption("zh-CN", t.zhCnLanguage)
        .setValue(this.plugin.settings.language)
        .onChange(async (value) => {
          this.plugin.settings.language = value as PluginLanguage;
          await this.plugin.saveSettings();
          this.display();
        })
    );
  }

  private renderConnection(containerEl: HTMLElement): void {
    const t = this.plugin.text();
    containerEl.createEl("h3", { text: t.connection });
    new Setting(containerEl)
      .setName(t.serverUrl)
      .setDesc(t.serverUrlDescription)
      .addText((text) =>
        text
          .setPlaceholder("https://sync.example.com/k_xxx/")
          .setValue(this.plugin.settings.serverUrl)
          .onChange(async (value) => {
            this.plugin.settings.serverUrl = value.trim();
            await this.plugin.saveSettings();
          })
      );

    new Setting(containerEl)
      .setName(t.deploymentKey)
      .setDesc(t.deploymentKeyDescription)
      .addText((text) =>
        text
          .setPlaceholder("k_xxx")
          .setValue(this.plugin.settings.deploymentKey)
          .onChange(async (value) => {
            this.plugin.settings.deploymentKey = value.trim();
            await this.plugin.saveSettings();
          })
      );

    new Setting(containerEl).setName(t.deviceName).addText((text) =>
      text.setValue(this.plugin.settings.deviceName).onChange(async (value) => {
        this.plugin.settings.deviceName = value.trim();
        await this.plugin.saveSettings();
      })
    );

    new Setting(containerEl).addButton((button) =>
      button
        .setButtonText(t.connect)
        .setCta()
        .onClick(async () => {
          try {
            const parsed = parseServerUrl(
              this.plugin.settings.serverUrl,
              this.plugin.settings.deploymentKey
            );
            this.plugin.settings.serverUrl = parsed.serverUrl;
            this.plugin.settings.deploymentKey = parsed.deploymentKey;
            await this.plugin.saveSettings();
            this.cfg = await this.plugin.api().config();
            new Notice(
              format(t.connectedToServer, { serverName: this.cfg.server_name })
            );
            this.display();
          } catch (error) {
            new Notice(error instanceof Error ? error.message : String(error));
          }
        })
    );

    if (this.cfg) {
      containerEl.createEl("p", {
        text: format(t.serverInfo, {
          serverName: this.cfg.server_name,
          version: this.cfg.version,
          registration: this.cfg.registration
        })
      });
    }
  }

  private renderAccount(containerEl: HTMLElement): void {
    const t = this.plugin.text();
    containerEl.createEl("h3", { text: t.account });
    if (this.plugin.settings.token) {
      containerEl.createEl("p", {
        text: format(t.loggedInAs, { username: this.plugin.settings.username })
      });
      new Setting(containerEl).addButton((button) =>
        button.setButtonText(t.logout).onClick(async () => {
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
        })
      );
      new Setting(containerEl).addButton((button) =>
        button
          .setButtonText(t.syncNowButton)
          .setCta()
          .onClick(() => void this.plugin.syncNowManual())
      );
      void this.renderAccountDetails(containerEl);
      return;
    }

    let username = "";
    let password = "";
    let inviteCode = "";
    new Setting(containerEl).setName(t.username).addText((text) =>
      text.onChange((value) => {
        username = value.trim();
      })
    );
    new Setting(containerEl).setName(t.password).addText((text) => {
      text.inputEl.type = "password";
      text.onChange((value) => {
        password = value;
      });
    });
    new Setting(containerEl).setName(t.inviteCode).addText((text) =>
      text.onChange((value) => {
        inviteCode = value.trim();
      })
    );
    new Setting(containerEl)
      .addButton((button) =>
        button
          .setButtonText(t.login)
          .setCta()
          .onClick(async () => this.login(username, password))
      )
      .addButton((button) =>
        button
          .setButtonText(t.register)
          .onClick(async () => this.register(username, password, inviteCode))
      );
  }

  private async login(username: string, password: string): Promise<void> {
    try {
      const response = await this.plugin
        .api()
        .login(username, password, this.plugin.settings.deviceName);
      this.plugin.settings.token = response.token;
      this.plugin.settings.userId = response.user_id;
      this.plugin.settings.username = response.username;
      await this.plugin.saveSettings();
      new Notice(this.plugin.text().loggedIn);
      this.display();
    } catch (error) {
      new Notice(error instanceof ApiError ? error.message : String(error));
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
      new Notice(error instanceof ApiError ? error.message : String(error));
    }
  }

  private async renderAccountDetails(containerEl: HTMLElement): Promise<void> {
    try {
      const current = this.plugin.text();
      const me = await this.plugin.api().me();
      containerEl.createEl("h4", { text: current.vaults });
      for (const vault of me.vaults) {
        const selected = this.plugin.settings.selectedVaultId === vault.id;
        new Setting(containerEl)
          .setName(vault.name)
          .setDesc(
            format(current.vaultSelectableSummary, {
              fileCount: vault.file_count,
              sizeBytes: vault.size_bytes,
              selected: selected ? current.selectedVaultSuffix : ""
            })
          )
          .addButton((button) =>
            button
              .setButtonText(selected ? current.selectedVaultButton : current.useVaultButton)
              .setDisabled(selected)
              .onClick(async () => {
                this.plugin.settings.selectedVaultId = vault.id;
                this.plugin.settings.selectedVaultName = vault.name;
                await this.plugin.saveSettings();
                new Notice(format(current.selectedVaultNotice, { name: vault.name }));
                this.display();
              })
          );
      }
      let vaultName = "";
      new Setting(containerEl)
        .setName(current.createVault)
        .setDesc(current.vaultNameDescription)
        .addText((text) =>
          text.setPlaceholder(current.vaultName).onChange((value) => {
            vaultName = value.trim();
          })
        )
        .addButton((button) =>
          button.setButtonText(current.createVault).onClick(async () => {
            if (!vaultName) {
              new Notice(current.vaultNameRequired);
              return;
            }
            try {
              const vault = await this.plugin.api().createVault(vaultName);
              this.plugin.settings.selectedVaultId = vault.id;
              this.plugin.settings.selectedVaultName = vault.name;
              await this.plugin.saveSettings();
              new Notice(format(current.createdVaultNotice, { name: vault.name }));
              this.display();
            } catch (error) {
              new Notice(
                error instanceof Error
                  ? `${current.createVaultFailed}: ${error.message}`
                  : `${current.createVaultFailed}: ${String(error)}`
              );
            }
          })
        );
      const tokens = await this.plugin.api().tokens();
      containerEl.createEl("h4", { text: current.tokens });
      const tokenList = containerEl.createEl("ul");
      for (const token of tokens) {
        tokenList.createEl("li", {
          text: `${token.device_name}${token.current ? current.currentDeviceSuffix : ""}`
        });
      }
    } catch (error) {
      containerEl.createEl("p", {
        text: error instanceof Error ? error.message : String(error),
        cls: "pkv-sync-error"
      });
    }
  }
}
