import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { MigrateModal } from "../src/ui/migrate-modal";
import { createMigrationHarness, targetScopeKey } from "./helpers/migration-fixture";
import { notices } from "./mocks/obsidian";

let opened: MigrateModal | null = null;

beforeEach(() => {
  opened = null;
  notices.length = 0;
  vi.spyOn(MigrateModal.prototype, "open").mockImplementation(function (this: MigrateModal) {
    opened = this;
    this.onOpen();
  });
});
afterEach(() => vi.restoreAllMocks());

function controls() {
  if (!opened) throw new Error("migration modal was not opened");
  return opened as unknown as {
    start(): Promise<void>;
    statusEl: HTMLElement;
    progressEl: HTMLElement;
  };
}

it("shows recovery failure without undoing the committed migration", async () => {
  const h = createMigrationHarness({ files: [{ path: "a.md", content: "alpha" }] });
  (h.plugin as unknown as { orchestratorInstance: { rebuild(): void } }).orchestratorInstance.rebuild = () => {
    throw new Error("engine recovery failed");
  };
  await h.plugin.openMigrationModal();
  const modal = controls();

  await modal.start();

  expect(h.savedSettings().selectedVaultId).toBe("vault-new");
  expect(h.savedIndex(targetScopeKey(h.oldSettings(), "vault-new")).files).toHaveProperty("a.md");
  expect(modal.statusEl.setText).toHaveBeenLastCalledWith(h.plugin.text().migrateFailed);
  expect(modal.progressEl.setText).toHaveBeenLastCalledWith(h.plugin.text().migrateEngineFailed);
  expect(modal.statusEl.setText).not.toHaveBeenCalledWith(h.plugin.text().migrateStageComplete);
});

it("does not render completion after a queued selection supersedes the migration", async () => {
  const h = createMigrationHarness({ files: [{ path: "a.md", content: "alpha" }] });
  await h.plugin.openMigrationModal();
  const modal = controls();
  const held = h.holdNextSave();
  const running = modal.start();
  await vi.waitFor(() => expect(h.saveCalls()).toBe(1));
  const switching = h.plugin.setSelectedVault("vault-later", "Later");
  held.release();
  await Promise.all([running, switching]);

  expect(h.savedSettings().selectedVaultId).toBe("vault-later");
  expect(h.savedIndex(targetScopeKey(h.oldSettings(), "vault-new")).files).toHaveProperty("a.md");
  expect(modal.statusEl.setText).toHaveBeenLastCalledWith(h.plugin.text().migrateFailed);
  expect(modal.progressEl.setText).toHaveBeenLastCalledWith(h.plugin.text().migrateScopeChanged);
  expect(modal.statusEl.setText).not.toHaveBeenCalledWith(h.plugin.text().migrateStageComplete);
});

it("renders completion when the migration remains current and engine recovery succeeds", async () => {
  const h = createMigrationHarness({ files: [{ path: "a.md", content: "alpha" }] });
  await h.plugin.openMigrationModal();
  const modal = controls();

  await modal.start();

  expect(h.savedSettings().selectedVaultId).toBe("vault-new");
  expect(h.events).toEqual(["invalidate", "rebuild"]);
  expect(modal.statusEl.setText).toHaveBeenLastCalledWith(h.plugin.text().migrateStageComplete);
});
