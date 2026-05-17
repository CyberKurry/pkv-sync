import { describe, expect, it, vi } from "vitest";
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
});

describe("delete vault", () => {
  it("deleting selected vault clears settings and invalidates engine", async () => {
    const invalidateSyncEngine = vi.fn();
    const saveSettings = vi.fn().mockResolvedValue(undefined);
    const deleteVault = vi.fn().mockResolvedValue(undefined);
    const settings = {
      selectedVaultId: "vault-1",
      selectedVaultName: "Test Vault"
    };

    const vault = mockVault();
    await deleteVault(vault.id);
    if (settings.selectedVaultId === vault.id) {
      settings.selectedVaultId = "";
      settings.selectedVaultName = "";
      invalidateSyncEngine();
    }
    await saveSettings();

    expect(deleteVault).toHaveBeenCalledWith("vault-1");
    expect(settings.selectedVaultId).toBe("");
    expect(settings.selectedVaultName).toBe("");
    expect(invalidateSyncEngine).toHaveBeenCalledTimes(1);
    expect(saveSettings).toHaveBeenCalledTimes(1);
  });

  it("deleting non-selected vault does not clear settings or invalidate engine", async () => {
    const invalidateSyncEngine = vi.fn();
    const saveSettings = vi.fn().mockResolvedValue(undefined);
    const deleteVault = vi.fn().mockResolvedValue(undefined);
    const settings = {
      selectedVaultId: "vault-other",
      selectedVaultName: "Other Vault"
    };

    const vault = mockVault();
    await deleteVault(vault.id);
    if (settings.selectedVaultId === vault.id) {
      settings.selectedVaultId = "";
      settings.selectedVaultName = "";
      invalidateSyncEngine();
    }
    await saveSettings();

    expect(deleteVault).toHaveBeenCalledWith("vault-1");
    expect(settings.selectedVaultId).toBe("vault-other");
    expect(invalidateSyncEngine).not.toHaveBeenCalled();
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
