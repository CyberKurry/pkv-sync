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
    const h = createMigrationHarness({});
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
    const h = createMigrationHarness({});
    const plugin = h.plugin as unknown as PKVSyncPlugin;
    await plugin.saveSettings({ rebuild: false });
    expect(notices).toEqual([]);
  });
});
