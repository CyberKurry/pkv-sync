import { describe, expect, it, vi } from "vitest";
import { PKVSyncSettingTab } from "../../src/ui/settings-tab";

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
