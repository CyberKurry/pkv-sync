import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ApiClient } from "../src/api/client";
import type { MeResponse } from "../src/api/types";
import { registerCommands } from "../src/commands";
import { createMigrationHarness } from "./helpers/migration-fixture";
import { notices } from "./mocks/obsidian";

function registerAccountCommand(h: ReturnType<typeof createMigrationHarness>, me: () => Promise<MeResponse>) {
  const handlers = new Map<string, () => unknown>();
  Object.assign(h.plugin, {
    addCommand(command: { id: string; callback?: () => unknown }) {
      if (command.callback) handlers.set(command.id, command.callback);
    }
  });
  h.plugin.apiForConnection = () => ({ me }) as unknown as ApiClient;
  registerCommands(h.plugin, h.plugin.text());
  const refresh = handlers.get("refresh-account");
  if (!refresh) throw new Error("refresh-account command was not registered");
  return refresh;
}

beforeEach(() => { notices.length = 0; });
afterEach(() => vi.restoreAllMocks());

it.each([
  { name: "a missing user id", settings: { userId: "" }, username: "old" },
  { name: "a changed username", settings: {}, username: "renamed" }
])("resumes sync after refreshing $name", async ({ settings, username }) => {
  const h = createMigrationHarness({ settings });
  const refresh = registerAccountCommand(h, async () => ({
    user_id: "user-old", username, is_admin: false, vaults: []
  }));

  await refresh();

  expect(h.plugin.settings.userId).toBe("user-old");
  expect(h.savedSettings().username).toBe(username);
  expect(h.events).toEqual(["invalidate", "rebuild"]);
  expect(notices).toContain(h.plugin.text().refreshedVaults.replace("{count}", "0"));
});

it("keeps account refresh behind the persistence recovery barrier", async () => {
  const h = createMigrationHarness({ settings: { userId: "" } });
  h.plugin.persistenceRecoveryRequired = true;
  const refresh = registerAccountCommand(h, async () => ({
    user_id: "user-old", username: "old", is_admin: false, vaults: []
  }));

  await refresh();

  expect(h.events).toEqual(["invalidate"]);
  expect(h.plugin.persistenceRecoveryRequired).toBe(true);
});

it("does not apply or resume an account response from an old connection", async () => {
  const h = createMigrationHarness();
  let release!: (value: MeResponse) => void;
  const response = new Promise<MeResponse>((resolve) => { release = resolve; });
  const me = vi.fn(() => response);
  const refresh = registerAccountCommand(h, me);
  const pending = refresh();
  await vi.waitFor(() => expect(me).toHaveBeenCalledTimes(1));
  await h.plugin.setConnectionPatch({ serverUrl: "https://new.example.test" });
  release({ user_id: "another-user", username: "another", is_admin: false, vaults: [] });
  await pending;

  expect(h.plugin.settings.username).toBe("old");
  expect(h.plugin.settings.userId).toBe("user-old");
  expect(h.events).toEqual(["invalidate"]);
  expect(notices).toEqual([]);
});

it("does not resume a new connection queued behind the account identity save", async () => {
  const h = createMigrationHarness({ settings: { userId: "" } });
  const refresh = registerAccountCommand(h, async () => ({
    user_id: "user-old", username: "old", is_admin: false, vaults: []
  }));
  const held = h.holdNextSave();
  const pending = refresh();
  await vi.waitFor(() => expect(h.saveCalls()).toBe(1));
  const switching = h.plugin.setConnectionPatch({ serverUrl: "https://new.example.test" });
  held.release();
  await Promise.all([pending, switching]);

  expect(h.plugin.settings.serverUrl).toBe("https://new.example.test");
  expect(h.events).toEqual(["invalidate", "invalidate"]);
  expect(notices).toEqual([]);
});

it("checks a queued recovery's expected connection inside the publication gate", async () => {
  const h = createMigrationHarness();
  const stamp = await h.plugin.captureConnection();
  const held = h.holdNextSave();
  const holding = h.plugin.setSelectedVault("holding", "Holding");
  await vi.waitFor(() => expect(h.saveCalls()).toBe(1));
  const switching = h.plugin.setConnectionPatch({ serverUrl: "https://new.example.test" });
  const resuming = h.plugin.resumeSyncForCurrentScope(stamp);
  held.release();
  await Promise.all([holding, switching, resuming]);

  expect(h.events).toEqual(["invalidate"]);
});
