import { describe, expect, it, vi } from "vitest";
import { Platform } from "obsidian";
import { ApiError } from "../../src/api/client";
import { PKVSyncSettingTab } from "../../src/ui/settings-tab";
import { DeleteVaultModal } from "../../src/ui/delete-vault-modal";
import { en } from "../../src/i18n/en";
import { ja } from "../../src/i18n/ja";
import { ko } from "../../src/i18n/ko";
import { zh } from "../../src/i18n/zh";
import { zhHant } from "../../src/i18n/zh-Hant";
import { notices } from "../mocks/obsidian";

function mockVault(overrides: Record<string, unknown> = {}) {
  return {
    id: "vault-1",
    user_id: "u1",
    name: "Test Vault",
    created_at: 1,
    last_sync_at: null as number | null,
    size_bytes: 0,
    file_count: 0,
    ...overrides
  };
}

describe("PKVSyncSettingTab connection state", () => {
  it("returns from login/register state to editable server settings", () => {
    const tab = Object.create(PKVSyncSettingTab.prototype) as {
      cfg: unknown;
      display: () => void;
      showConnectionSettings: () => void;
    };
    tab.cfg = { server_name: "Self-hosted" };
    tab.display = vi.fn();

    tab.showConnectionSettings();

    expect(tab.cfg).toBeNull();
    expect(tab.display).toHaveBeenCalledTimes(1);
  });

  it("marks the settings root as mobile when Obsidian reports a phone layout", () => {
    const previous = {
      isMobile: Platform.isMobile,
      isMobileApp: Platform.isMobileApp,
      isPhone: Platform.isPhone
    };
    Platform.isMobile = true;
    Platform.isMobileApp = true;
    Platform.isPhone = true;

    const shell = mockElement();
    const panel = mockElement();
    const containerEl = mockElement();
    containerEl.createDiv.mockReturnValueOnce(shell);
    shell.createDiv.mockReturnValueOnce(panel);

    const tab = new PKVSyncSettingTab(
      { vault: { getFiles: () => [] } } as never,
      {
        settings: {
          token: "",
          serverUrl: "",
          deploymentKey: "",
          deviceName: "Phone",
          timezone: "Asia/Shanghai",
          language: "auto",
          themeMode: "dark",
          checkForUpdates: true,
          updateSource: "server",
          lastUpdateCheckAt: null
        },
        text: () => ({
          settingsTitle: "PKV Sync",
          language: "Language",
          autoLanguage: "Auto",
          englishLanguage: "English",
          zhCnLanguage: "Simplified Chinese",
          zhHantLanguage: "Traditional Chinese",
          japaneseLanguage: "Japanese",
          koreanLanguage: "Korean",
          needsReviewSuffix: "(needs review)",
          translationNeedsReview: "This language is community-translated.",
          helpTranslate: "Help translate",
          themeMode: "Theme",
          themeAuto: "Auto",
          themeLight: "Light",
          themeDark: "Dark",
          connection: "Connection",
          serverUrl: "Server URL",
          deploymentKey: "Deployment Key",
          deviceName: "Device Name",
          timezone: "Timezone",
          connect: "Connect",
          settingsUpdateSection: "Updates",
          currentVersion: "Current version: {version}",
          lastUpdateCheck: "Last checked: {time}",
          neverSynced: "Never",
          checkForUpdates: "Check for updates",
          updateSource: "Update source",
          updateSourceServer: "Server",
          updateSourceGitHub: "GitHub",
          updateCheckNow: "Check now",
          updateNow: "Update now",
          conflictFiles: "Conflict files",
          conflictFilesSummary: "{count} conflict files",
          deleteConflictsButton: "Delete conflicts"
        }),
        saveSettings: vi.fn(),
        api: vi.fn()
      } as never
    );
    tab.containerEl = containerEl as never;

    try {
      tab.display();

      expect(containerEl.toggleClass).toHaveBeenCalledWith("is-mobile", true);
      expect(containerEl.toggleClass).toHaveBeenCalledWith("is-phone", true);
      expect(containerEl.toggleClass).toHaveBeenCalledWith("is-light-override", false);
      expect(containerEl.toggleClass).toHaveBeenCalledWith("is-dark-override", true);
    } finally {
      Platform.isMobile = previous.isMobile;
      Platform.isMobileApp = previous.isMobileApp;
      Platform.isPhone = previous.isPhone;
    }
  });

  it("shows localized first-run setup guidance when connect receives setup_required", async () => {
    const config = vi
      .fn()
      .mockRejectedValue(
        new ApiError(403, "setup_required", "Initial setup required")
      );
    const saveSettings = vi.fn().mockResolvedValue(undefined);
    const containerEl = new MockElement("div");
    const setupRequiredNotice =
      "Open the server URL in a browser to complete first-run setup, then connect again.";
    const plugin = {
      settings: {
        token: "",
        serverUrl: "https://sync.example.com",
        deploymentKey: "k_abc",
        deviceName: "Laptop",
        timezone: "Asia/Shanghai",
        language: "auto",
        themeMode: "auto"
      },
      text: () => ({
        ...en,
        setupRequiredNotice
      }),
      saveSettings,
      api: () => ({ config })
    };
    const tab = new PKVSyncSettingTab(
      { vault: { getFiles: () => [] } } as never,
      plugin as never
    );
    tab.containerEl = containerEl as never;
    notices.length = 0;

    tab.display();
    await containerEl.clickButton("Connect");

    expect(config).toHaveBeenCalledTimes(1);
    expect(notices.at(-1)).toBe(setupRequiredNotice);
  });
});

describe("delete vault", () => {
  function buildTab(settingsOverride: Record<string, unknown> = {}) {
    const deleteVault = vi.fn().mockResolvedValue(undefined);
    const saveSettings = vi.fn().mockResolvedValue(undefined);
    const invalidateSyncEngine = vi.fn();
    const settings = {
      selectedVaultId: "vault-1",
      selectedVaultName: "Test Vault",
      ...settingsOverride
    };
    const plugin = {
      settings,
      api: () => ({ deleteVault }),
      saveSettings,
      invalidateSyncEngine,
      text: () => ({ deletedVaultNotice: "Deleted {name}" })
    };

    const tab = Object.create(PKVSyncSettingTab.prototype) as {
      plugin: typeof plugin;
      display: () => void;
      deleteVaultAndRefresh: (vault: ReturnType<typeof mockVault>) => Promise<void>;
    };
    tab.plugin = plugin;
    tab.display = vi.fn();

    return { tab, plugin, deleteVault, saveSettings, invalidateSyncEngine, settings };
  }

  it("deleting selected vault calls API, clears settings, invalidates engine, refreshes display", async () => {
    const { tab, deleteVault, saveSettings, invalidateSyncEngine, settings } =
      buildTab();
    const vault = mockVault();
    notices.length = 0;

    await tab.deleteVaultAndRefresh(vault);

    expect(deleteVault).toHaveBeenCalledWith("vault-1");
    expect(settings.selectedVaultId).toBe("");
    expect(settings.selectedVaultName).toBe("");
    expect(invalidateSyncEngine).toHaveBeenCalledTimes(1);
    expect(saveSettings).toHaveBeenCalledTimes(1);
    expect(tab.display).toHaveBeenCalledTimes(1);
    expect(notices.at(-1)).toBe("Deleted Test Vault");
  });

  it("deleting non-selected vault preserves selection and does not invalidate engine", async () => {
    const { tab, deleteVault, saveSettings, invalidateSyncEngine, settings } =
      buildTab({
        selectedVaultId: "vault-other",
        selectedVaultName: "Other Vault"
      });
    const vault = mockVault();

    await tab.deleteVaultAndRefresh(vault);

    expect(deleteVault).toHaveBeenCalledWith("vault-1");
    expect(settings.selectedVaultId).toBe("vault-other");
    expect(settings.selectedVaultName).toBe("Other Vault");
    expect(invalidateSyncEngine).not.toHaveBeenCalled();
    expect(saveSettings).toHaveBeenCalledTimes(1);
    expect(tab.display).toHaveBeenCalledTimes(1);
  });

  it("does not clear settings or refresh when API rejects", async () => {
    const { tab, saveSettings, invalidateSyncEngine, settings } = buildTab();
    const failingApi = vi.fn().mockRejectedValue(new Error("boom"));
    tab.plugin.api = () => ({ deleteVault: failingApi });
    const vault = mockVault();

    await expect(tab.deleteVaultAndRefresh(vault)).rejects.toThrow("boom");
    expect(settings.selectedVaultId).toBe("vault-1");
    expect(invalidateSyncEngine).not.toHaveBeenCalled();
    expect(saveSettings).not.toHaveBeenCalled();
    expect(tab.display).not.toHaveBeenCalled();
  });

  it("DeleteVaultModal confirms delete and shows notice on API error", async () => {
    const labels = {
      deleteVaultModalTitle: "Delete vault",
      deleteVaultModalBody: "Delete \"{name}\"",
      deleteVaultConfirmPrompt: "Type \"{name}\"",
      deleteVaultConfirmButton: "Delete",
      deleteVaultCancelButton: "Cancel",
      deleteVaultFailed: "Failed"
    } as any;
    const onConfirm = vi.fn().mockRejectedValue(new Error("Server error"));
    const vault = mockVault();

    const modal = new DeleteVaultModal({} as any, vault, labels, onConfirm);
    modal.open();

    expect(modal.contentEl.addClass).toHaveBeenCalledWith("pkvsync-delete-vault-modal");

    notices.length = 0;
    await (modal as any).handleDelete();

    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(notices.length).toBe(1);
    expect(notices[0]).toContain("Failed");
  });
});

describe("vault sync allowlist settings", () => {
  function buildAllowlistTab() {
    const getVaultSettings = vi.fn().mockResolvedValue({
      extra_sync_globs: [".obsidian/themes/**"]
    });
    const putVaultSettings = vi.fn().mockResolvedValue(undefined);
    const plugin = {
      settings: {
        selectedVaultId: "vault-1",
        selectedVaultName: "Test Vault"
      },
      api: () => ({ getVaultSettings, putVaultSettings }),
      text: () => ({
        vaultSyncAllowlist: "Dotfile sync allowlist",
        vaultSyncAllowlistHint: "One glob per line.",
        vaultSyncAllowlistPlaceholder: ".obsidian/themes/**",
        vaultSyncAllowlistStarterButton: "Apply recommended starter list",
        vaultSyncAllowlistSaveButton: "Save",
        vaultSyncAllowlistSaved: "Saved dotfile sync allowlist",
        vaultSyncAllowlistLoadFailed: "Failed to load dotfile sync allowlist",
        vaultSyncAllowlistSaveFailed: "Failed to save dotfile sync allowlist"
      })
    };
    const tab = Object.create(PKVSyncSettingTab.prototype) as {
      plugin: typeof plugin;
      renderVaultSyncAllowlist: (body: MockElement) => Promise<void>;
    };
    tab.plugin = plugin;
    return { tab, getVaultSettings, putVaultSettings };
  }

  it("loads vault settings into a textarea, applies the starter list, and saves globs", async () => {
    const { tab, getVaultSettings, putVaultSettings } = buildAllowlistTab();
    const body = new MockElement("div");
    notices.length = 0;

    await tab.renderVaultSyncAllowlist(body);

    const actions = body.findByClass("pkv-sync-allowlist-actions");
    expect(actions?.cls).toContain("pkv-sync-button-row");

    const textarea = body.find("textarea");
    expect(getVaultSettings).toHaveBeenCalledWith("vault-1");
    expect(textarea?.value).toBe(".obsidian/themes/**");

    body.clickButton("Apply recommended starter list");
    expect(textarea?.value).toBe(
      [".obsidian/themes/**", ".obsidian/snippets/**"].join("\n")
    );

    await body.clickButton("Save");

    expect(putVaultSettings).toHaveBeenCalledWith("vault-1", {
      extra_sync_globs: [".obsidian/themes/**", ".obsidian/snippets/**"]
    });
    expect(notices.at(-1)).toBe("Saved dotfile sync allowlist");
  });

  it("has localized strings for the allowlist editor", () => {
    const keys = [
      "vaultSyncAllowlist",
      "vaultSyncAllowlistHint",
      "vaultSyncAllowlistPlaceholder",
      "vaultSyncAllowlistStarterButton",
      "vaultSyncAllowlistSaveButton",
      "vaultSyncAllowlistSaved",
      "vaultSyncAllowlistLoadFailed",
      "vaultSyncAllowlistSaveFailed"
    ] as const;

    for (const key of keys) {
      expect(en[key]).toEqual(expect.any(String));
      expect(en[key].length).toBeGreaterThan(0);
      expect(zh[key]).toEqual(expect.any(String));
      expect(zh[key].length).toBeGreaterThan(0);
    }
    expect(zh.vaultSyncAllowlistHint).toContain("。");
  });
  it("has localized first-run setup guidance in every plugin locale", () => {
    for (const bundle of [en, zh, zhHant, ja, ko]) {
      expect(bundle.setupRequiredNotice).toEqual(expect.any(String));
      expect(bundle.setupRequiredNotice.length).toBeGreaterThan(0);
    }
  });
});

describe("plugin updates settings", () => {
  it("renders available update actions and applies the selected update", async () => {
    const applyPluginUpdate = vi.fn().mockResolvedValue(undefined);
    const checkForPluginUpdates = vi.fn().mockResolvedValue(null);
    const saveSettings = vi.fn().mockResolvedValue(undefined);
    const display = vi.fn();
    const plugin = {
      manifest: { version: "0.8.0" },
      settings: {
        checkForUpdates: true,
        updateSource: "server",
        lastUpdateCheckAt: 1_000,
        timezone: "Asia/Shanghai"
      },
      availableUpdate: {
        version: "0.8.1",
        releaseNotesUrl:
          "https://github.com/cyberkurry/pkv-sync/releases/tag/v0.8.1"
      },
      text: () => en,
      saveSettings,
      scheduleUpdateChecks: vi.fn(),
      checkForPluginUpdates,
      applyPluginUpdate
    };
    const tab = Object.create(PKVSyncSettingTab.prototype) as {
      plugin: typeof plugin;
      display: () => void;
      renderUpdates: (body: MockElement) => void;
    };
    tab.plugin = plugin;
    tab.display = display;
    const body = new MockElement("div");

    tab.renderUpdates(body);

    expect(body.textContent()).toContain("Current version: 0.8.0");
    expect(body.textContent()).toContain("v0.8.1 available");

    await body.clickButton("Update now");

    expect(applyPluginUpdate).toHaveBeenCalledWith(plugin.availableUpdate);
  });

  it("has localized update strings in every plugin locale", () => {
    const keys = [
      "settingsUpdateSection",
      "currentVersion",
      "checkForUpdates",
      "lastUpdateCheck",
      "updateCheckNow",
      "updateAvailable",
      "updateReleaseNotes",
      "updateNow",
      "updateSourceServer",
      "updateSourceGitHub",
      "checkForPluginUpdatesCommand",
      "updateSuccess",
      "updateFailed",
      "updateSha256Mismatch",
      "updateUpToDate"
    ] as const;

    for (const bundle of [en, zh, zhHant, ja, ko]) {
      for (const key of keys) {
        expect(bundle[key]).toEqual(expect.any(String));
        expect(bundle[key].length).toBeGreaterThan(0);
      }
    }
  });
});

describe("device list settings", () => {
  it("renders connected devices as card rows with current-device badge", () => {
    const plugin = {
      text: () => ({
        tokens: "Devices",
        currentDeviceSuffix: " (current)"
      })
    };
    const tab = Object.create(PKVSyncSettingTab.prototype) as {
      plugin: typeof plugin;
      renderDevices: (body: MockElement, tokens: unknown[]) => void;
    };
    tab.plugin = plugin;
    const body = new MockElement("div");

    tab.renderDevices(body, [
      {
        id: "token-1",
        device_name: "Laptop",
        current: true
      },
      {
        id: "token-2",
        device_name: "Android",
        current: false
      }
    ]);

    const list = body.findByClass("pkv-sync-device-list");
    expect(list?.tag).toBe("div");
    expect(body.findAllByClass("pkv-sync-device-card")).toHaveLength(2);
    expect(body.findAllByClass("pkv-sync-device-status")).toHaveLength(2);
    expect(body.findAllByClass("pkv-sync-device-badge")).toHaveLength(1);
    expect(body.textContent()).toContain("Laptop");
    expect(body.textContent()).toContain("Android");
    expect(body.textContent()).toContain("(current)");
  });
});

describe("language selector settings", () => {
  it("renders language selection after login", () => {
    const body = new MockElement("div");
    const plugin = {
      settings: {
        language: "zh-Hant",
        themeMode: "auto",
        lastSyncSuccessAt: null
      },
      text: () => en,
      saveSettings: vi.fn().mockResolvedValue(undefined),
      api: () => ({
        me: vi.fn(),
        tokens: vi.fn()
      })
    };
    const tab = Object.create(PKVSyncSettingTab.prototype) as {
      plugin: typeof plugin;
      syncTimeExpanded: boolean;
      renderId: number;
      renderSynced: (panel: MockElement, renderId: number) => void;
      syncDetailTime: () => string;
      isMobileLayout: () => boolean;
    };
    tab.plugin = plugin;
    tab.syncTimeExpanded = false;
    tab.renderId = 1;
    tab.syncDetailTime = () => "";
    tab.isMobileLayout = () => false;

    tab.renderSynced(body, 1);

    expect(body.findByClass("pkv-sync-language-select")).toBeTruthy();
    expect(body.textContent()).toContain("Language");
  });
});

describe("theme mode settings", () => {
  it("renders theme mode as one icon button that cycles to the next mode", async () => {
    const saveSettings = vi.fn().mockResolvedValue(undefined);
    const settings = {
      language: "auto",
      themeMode: "auto",
      lastSyncSuccessAt: null
    };
    const body = new MockElement("div");
    const plugin = {
      settings,
      text: () => en,
      saveSettings,
      api: () => ({
        me: vi.fn(),
        tokens: vi.fn()
      })
    };
    const tab = Object.create(PKVSyncSettingTab.prototype) as {
      plugin: typeof plugin;
      syncTimeExpanded: boolean;
      renderId: number;
      display: () => void;
      renderSynced: (panel: MockElement, renderId: number) => void;
      syncDetailTime: () => string;
      isMobileLayout: () => boolean;
    };
    tab.plugin = plugin;
    tab.syncTimeExpanded = false;
    tab.renderId = 1;
    tab.display = vi.fn();
    tab.syncDetailTime = () => "";
    tab.isMobileLayout = () => false;

    tab.renderSynced(body, 1);

    const button = body.findByClass("pkv-sync-theme-button");
    expect(button).toBeTruthy();
    expect(button?.attrs["data-theme-mode"]).toBe("auto");
    expect(button?.attrs["aria-label"]).toBe("Theme: Auto");
    expect(body.findAllByClass("pkv-sync-theme-icon")).toHaveLength(1);
    expect(body.textContent()).toContain("Theme");
    expect(body.textContent()).toContain("Auto");

    await body.clickButton("Auto");

    expect(settings.themeMode).toBe("light");
    expect(saveSettings).toHaveBeenCalledWith({ rebuild: false });
    expect(tab.display).toHaveBeenCalledTimes(1);
  });
});

function mockElement(): any {
  return {
    empty: vi.fn(),
    addClass: vi.fn(),
    removeClass: vi.fn(),
    toggleClass: vi.fn(),
    createDiv: vi.fn(() => mockElement()),
    createEl: vi.fn(() => mockElement()),
    createSpan: vi.fn(() => mockElement()),
    setText: vi.fn(),
    setAttr: vi.fn(),
    addEventListener: vi.fn()
  };
}

class MockElement {
  children: MockElement[] = [];
  listeners = new Map<string, Array<() => void | Promise<void>>>();
  value = "";
  disabled = false;
  text = "";
  cls = "";
  attrs: Record<string, string> = {};

  constructor(public tag: string) {}

  empty(): void {
    this.children = [];
    this.text = "";
  }

  addClass(): void {}

  removeClass(): void {}

  toggleClass(): void {}

  setAttr(name: string, value: string): void {
    this.attrs[name] = value;
  }

  createDiv(options: { cls?: string; text?: string } = {}): MockElement {
    return this.createChild("div", options);
  }

  createEl(
    tag: string,
    options: { cls?: string; text?: string; attr?: Record<string, string> } = {}
  ): MockElement {
    const child = this.createChild(tag, options);
    child.attrs = { ...(options.attr ?? {}) };
    if (options.attr?.placeholder) child.value = "";
    return child;
  }

  createSpan(options: { text?: string; cls?: string } = {}): MockElement {
    return this.createChild("span", options);
  }

  setText(text: string): void {
    this.text = text;
  }

  addEventListener(event: string, callback: () => void | Promise<void>): void {
    const list = this.listeners.get(event) ?? [];
    list.push(callback);
    this.listeners.set(event, list);
  }

  find(tag: string): MockElement | undefined {
    if (this.tag === tag) return this;
    for (const child of this.children) {
      const found = child.find(tag);
      if (found) return found;
    }
    return undefined;
  }

  findByClass(cls: string): MockElement | undefined {
    if (this.cls.split(/\s+/).includes(cls)) return this;
    for (const child of this.children) {
      const found = child.findByClass(cls);
      if (found) return found;
    }
    return undefined;
  }

  findAllByClass(cls: string): MockElement[] {
    return [
      ...(this.cls.split(/\s+/).includes(cls) ? [this] : []),
      ...this.children.flatMap((child) => child.findAllByClass(cls))
    ];
  }

  async clickButton(text: string): Promise<void> {
    const button = this.findButton(text);
    if (!button) throw new Error(`Button not found: ${text}`);
    for (const listener of button.listeners.get("click") ?? []) {
      await listener();
    }
    await Promise.resolve();
  }

  textContent(): string {
    return [this.text, ...this.children.map((child) => child.textContent())]
      .filter(Boolean)
      .join(" ");
  }

  private findButton(text: string): MockElement | undefined {
    if (this.tag === "button" && this.textContent() === text) return this;
    for (const child of this.children) {
      const found = child.findButton(text);
      if (found) return found;
    }
    return undefined;
  }

  private createChild(
    tag: string,
    options: { cls?: string; text?: string; attr?: Record<string, string> } = {}
  ): MockElement {
    const child = new MockElement(tag);
    child.cls = options.cls ?? "";
    child.text = options.text ?? "";
    child.attrs = { ...(options.attr ?? {}) };
    this.children.push(child);
    return child;
  }
}
