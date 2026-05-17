import { describe, expect, it, vi } from "vitest";
import { Platform } from "obsidian";
import { PKVSyncSettingTab } from "../../src/ui/settings-tab";
import { DeleteVaultModal } from "../../src/ui/delete-vault-modal";
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
          language: "auto"
        },
        text: () => ({
          settingsTitle: "PKV Sync",
          language: "Language",
          autoLanguage: "Auto",
          englishLanguage: "English",
          zhCnLanguage: "Simplified Chinese",
          connection: "Connection",
          serverUrl: "Server URL",
          deploymentKey: "Deployment Key",
          deviceName: "Device Name",
          timezone: "Timezone",
          connect: "Connect",
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
    } finally {
      Platform.isMobile = previous.isMobile;
      Platform.isMobileApp = previous.isMobileApp;
      Platform.isPhone = previous.isPhone;
    }
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
    addEventListener: vi.fn()
  };
}
