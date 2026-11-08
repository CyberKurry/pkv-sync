import { beforeEach, describe, expect, it } from "vitest";
import { notices } from "./mocks/obsidian";
import PKVSyncPlugin from "../src/main";
import { createMigrationHarness } from "./helpers/migration-fixture";
import { en } from "../src/i18n/en";

/**
 * R8-4: a settings-write failure (disk full, permissions) used to escape
 * saveSettings as an unhandled rejection — every `void saveSettings()`
 * call site (theme toggle, timezone select, connection-field blur...)
 * left memory settings changed, disk unwritten, and the user uninformed.
 * The fix surfaces the failure through the localized notice and still
 * rethrows so awaited callers keep their error path.
 */

describe("saveSettings surfaces write failures (R8-4)", () => {
  beforeEach(() => {
    notices.length = 0;
  });

  it("shows a localized notice and rethrows when the write fails", async () => {
    const h = createMigrationHarness({ settings: { language: "en" } });
    h.setSave(async () => {
      throw new Error("ENOSPC: no space left on device");
    });
    const plugin = h.plugin as unknown as PKVSyncPlugin & {
      text(): typeof en;
    };
    (plugin as unknown as { text: () => typeof en }).text = () => en;

    plugin.settings.themeMode = "dark";
    await expect(
      plugin.saveSettings({ rebuild: false })
    ).rejects.toThrow(/ENOSPC/);

    expect(notices).toEqual([
      'Could not save settings. The change is not persisted. Reason: ENOSPC: no space left on device'
    ]);
  });

  it("no notice on a successful save", async () => {
    const h = createMigrationHarness({ settings: { language: "en" } });
    const plugin = h.plugin as unknown as PKVSyncPlugin;
    await plugin.saveSettings({ rebuild: false });
    expect(notices).toEqual([]);
  });
});

describe("saveSettingsPatch surfaces write failures (R9-5)", () => {
  beforeEach(() => {
    notices.length = 0;
  });

  it("shows a localized notice and rethrows when the patch write fails", async () => {
    const h = createMigrationHarness({ settings: { language: "en" } });
    h.setSave(async () => {
      throw new Error("EACCES: permission denied");
    });
    const raw = h.plugin as unknown as {
      saveSettingsPatch: (
        patch: Partial<{ deviceName: string }>,
        options?: { rebuild?: boolean }
      ) => Promise<void>;
      settings: { deviceName: string };
    };
    const plugin = raw as unknown as PKVSyncPlugin;
    const saveSettingsPatch = raw.saveSettingsPatch.bind(h.plugin);

    await expect(
      saveSettingsPatch({ deviceName: "renamed" }, { rebuild: false })
    ).rejects.toThrow(/EACCES/);

    expect(notices.some((n) => n.includes("Could not save settings"))).toBe(
      true
    );
    // Memory was still updated (the caller decides whether to roll back);
    // the disk was not.
    expect(plugin.settings.deviceName).toBe("renamed");
  });

  it("does not fail the pass when a bookkeeping patch write fails", async () => {
    // recordSyncSuccess's contract (R9-5): engine.ts awaits onSyncSuccess
    // right after setting status "connected"; a throw there would rewrite
    // a successful pass into an error status. Bookkeeping writes report
    // the failure (notice) but resolve.
    const h = createMigrationHarness({ settings: { language: "en" } });
    h.setSave(async () => {
      throw new Error("EROFS: read-only file system");
    });
    const plugin = h.plugin as unknown as PKVSyncPlugin;

    await expect(
      plugin.recordSyncSuccess(plugin.syncGeneration)
    ).resolves.toBeUndefined();

    expect(notices.some((n) => n.includes("Could not save settings"))).toBe(
      true
    );
  });

  it("recordSyncSuccess does not fail a successful pass when the write fails", async () => {
    // The report's核心 case: engine.ts awaits onSyncSuccess AFTER setting
    // status "connected"; a throw there rewrites a successful pass into an
    // error status. The write failure must be visible (notice) but not
    // fail the pass.
    const h = createMigrationHarness({ settings: { language: "en" } });
    h.setSave(async () => {
      throw new Error("EROFS: read-only file system");
    });
    const plugin = h.plugin as unknown as PKVSyncPlugin;

    await expect(plugin.recordSyncSuccess(plugin.syncGeneration)).resolves.toBeUndefined();

    expect(notices.some((n) => n.includes("Could not save settings"))).toBe(
      true
    );
  });

  it("no notice when the patch write succeeds", async () => {
    const h = createMigrationHarness({});
    const plugin = h.plugin as unknown as PKVSyncPlugin;
    await plugin.checkForPluginUpdates(false);
    expect(notices).toEqual([]);
  });
});
