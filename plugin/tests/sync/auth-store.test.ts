import { describe, expect, it, vi } from "vitest";
import {
  AuthStore,
  authFromSettings,
  migrateAuth,
  type AuthData,
  type MigrationResult,
  type SafeStorageLike
} from "../../src/sync/auth-store";

function makeLocalStorage() {
  const store = new Map<string, unknown>();
  return {
    load: vi.fn((key: string) => (store.has(key) ? store.get(key) : null)),
    save: vi.fn((key: string, data: unknown) => {
      if (data === null) store.delete(key);
      else store.set(key, data);
    }),
    store
  };
}

const SAMPLE: AuthData = {
  deviceId: "dev-1",
  token: "tok-1",
  serverUrl: "https://sync.example.com",
  deploymentKey: "dk-1",
  userId: "user-1"
};

function makeSafeStorage(): SafeStorageLike & {
  encrypted: string[];
  decrypted: string[];
} {
  const safe = {
    encrypted: [] as string[],
    decrypted: [] as string[],
    isEncryptionAvailable: vi.fn(() => true),
    encryptString: vi.fn((plain: string) => {
      safe.encrypted.push(plain);
      return `sealed:${Buffer.from(plain, "utf8").toString("base64")}`;
    }),
    decryptString: vi.fn((ciphertext: string) => {
      const encoded = ciphertext.replace(/^sealed:/, "");
      const plain = Buffer.from(encoded, "base64").toString("utf8");
      safe.decrypted.push(plain);
      return plain;
    })
  };
  return safe;
}

describe("AuthStore", () => {
  it("returns null when nothing stored", () => {
    const ls = makeLocalStorage();
    const auth = new AuthStore(ls.load, ls.save);
    expect(auth.load()).toBeNull();
  });

  it("round-trips an AuthData object through a single key", () => {
    const ls = makeLocalStorage();
    const auth = new AuthStore(ls.load, ls.save);
    auth.save(SAMPLE);
    expect(ls.save).toHaveBeenCalledWith("pkv-sync-auth", SAMPLE);
    expect(auth.load()).toEqual(SAMPLE);
  });

  it("clear() removes the key", () => {
    const ls = makeLocalStorage();
    const auth = new AuthStore(ls.load, ls.save);
    auth.save(SAMPLE);
    auth.clear();
    expect(auth.load()).toBeNull();
    expect(ls.store.has("pkv-sync-auth")).toBe(false);
  });

  it("ignores a stored value missing required fields (treats as null)", () => {
    const ls = makeLocalStorage();
    ls.store.set("pkv-sync-auth", { token: "x" }); // no deviceId
    const auth = new AuthStore(ls.load, ls.save);
    expect(auth.load()).toBeNull();
  });

  it("ignores malformed stored auth field types", () => {
    const ls = makeLocalStorage();
    ls.store.set("pkv-sync-auth", {
      deviceId: "dev-1",
      token: 123,
      serverUrl: ["https://sync.example.com"],
      deploymentKey: { value: "dk" },
      userId: false
    });
    const auth = new AuthStore(ls.load, ls.save);
    expect(auth.load()).toBeNull();
  });

  it("stores only an encrypted safeStorage envelope when secure storage is available", () => {
    const ls = makeLocalStorage();
    const safe = makeSafeStorage();
    const auth = new AuthStore(ls.load, ls.save, safe);

    auth.save(SAMPLE);

    const stored = ls.store.get("pkv-sync-auth");
    expect(stored).toEqual({
      version: 1,
      kind: "electron-safe-storage",
      ciphertext: expect.any(String)
    });
    expect(JSON.stringify(stored)).not.toContain("tok-1");
    expect(JSON.stringify(stored)).not.toContain("dk-1");
    expect(safe.encrypted[0]).toContain("tok-1");
    expect(auth.load()).toEqual(SAMPLE);
  });

  it("migrates legacy plaintext localStorage auth into an encrypted envelope", () => {
    const ls = makeLocalStorage();
    ls.store.set("pkv-sync-auth", SAMPLE);
    const safe = makeSafeStorage();
    const auth = new AuthStore(ls.load, ls.save, safe);

    expect(auth.load()).toEqual(SAMPLE);

    const stored = ls.store.get("pkv-sync-auth");
    expect(JSON.stringify(stored)).not.toContain("tok-1");
    expect(stored).toMatchObject({
      version: 1,
      kind: "electron-safe-storage"
    });
  });
});

describe("authFromSettings — logout keeps device identity", () => {
  it("logged-in settings → full auth blob", () => {
    expect(authFromSettings({
      deviceId: "dev-keep", token: "tok", serverUrl: "https://s", deploymentKey: "dk", userId: "u"
    })).toEqual({ deviceId: "dev-keep", token: "tok", serverUrl: "https://s", deploymentKey: "dk", userId: "u" });
  });

  it("post-logout settings (token/userId emptied) keep deviceId + server, null the credentials", () => {
    const auth = authFromSettings({
      deviceId: "dev-keep", token: "", serverUrl: "https://s", deploymentKey: "dk", userId: ""
    });
    expect(auth.deviceId).toBe("dev-keep");
    expect(auth.serverUrl).toBe("https://s");
    expect(auth.deploymentKey).toBe("dk");
    expect(auth.token).toBeNull();
    expect(auth.userId).toBeNull();
  });
});

describe("migrateAuth", () => {
  const legacySettings = {
    deviceId: "dev-1",
    token: "tok-1",
    serverUrl: "https://s",
    deploymentKey: "dk",
    userId: "u",
    username: "alice",
    deviceName: "Laptop"
  };

  it("fresh install (no legacy auth) → fresh-install, no writes", () => {
    const ls = makeLocalStorage();
    const auth = new AuthStore(ls.load, ls.save);
    const result = migrateAuth(auth, { settings: { deviceName: "Laptop" } });
    expect(result.kind).toBe("fresh-install");
    expect(ls.store.has("pkv-sync-auth")).toBe(false);
    expect(result.strippedData).toBeNull();
  });

  it("legacy auth present → migrated, localStorage filled, returns stripped data.json", () => {
    const ls = makeLocalStorage();
    const auth = new AuthStore(ls.load, ls.save);
    const data = { settings: { ...legacySettings }, syncIndexes: { a: { lastSyncedCommit: "c", files: {} } } };
    const result = migrateAuth(auth, data);

    expect(result.kind).toBe("migrated");
    expect(auth.load()).toEqual({
      deviceId: "dev-1", token: "tok-1", serverUrl: "https://s", deploymentKey: "dk", userId: "u"
    });
    const s = (result.strippedData as any).settings;
    expect(s.deviceId).toBeUndefined();
    expect(s.token).toBeUndefined();
    expect(s.serverUrl).toBeUndefined();
    expect(s.deploymentKey).toBeUndefined();
    expect(s.userId).toBeUndefined();
    expect(s.username).toBe("alice");
    expect(s.deviceName).toBe("Laptop");
    expect((result.strippedData as any).syncIndexes).toEqual({ a: { lastSyncedCommit: "c", files: {} } });
  });

  it("legacy top-level auth is migrated and stripped from data.json", () => {
    const ls = makeLocalStorage();
    const auth = new AuthStore(ls.load, ls.save);
    const data = { ...legacySettings, settings: { deviceName: "Laptop" } };
    const result = migrateAuth(auth, data);

    expect(result.kind).toBe("migrated");
    expect(auth.load()).toEqual({
      deviceId: "dev-1", token: "tok-1", serverUrl: "https://s", deploymentKey: "dk", userId: "u"
    });
    const stripped = result.strippedData as Record<string, unknown>;
    expect(stripped.deviceId).toBeUndefined();
    expect(stripped.token).toBeUndefined();
    expect(stripped.serverUrl).toBeUndefined();
    expect(stripped.deploymentKey).toBeUndefined();
    expect(stripped.userId).toBeUndefined();
    expect((stripped.settings as Record<string, unknown>).deviceName).toBe("Laptop");
  });

  it("already migrated (localStorage has auth) but data.json still has residue → cleanup, strips residue", () => {
    const ls = makeLocalStorage();
    const auth = new AuthStore(ls.load, ls.save);
    auth.save({ deviceId: "dev-1", token: "tok-1", serverUrl: "https://s", deploymentKey: "dk", userId: "u" });
    const data = { settings: { ...legacySettings } };
    const result = migrateAuth(auth, data);
    expect(result.kind).toBe("already-migrated");
    expect((result.strippedData as any).settings.token).toBeUndefined();
    expect((result.strippedData as any).settings.username).toBe("alice");
  });

  it("already migrated top-level auth residue is stripped from data.json", () => {
    const ls = makeLocalStorage();
    const auth = new AuthStore(ls.load, ls.save);
    auth.save({ deviceId: "dev-1", token: "tok-1", serverUrl: "https://s", deploymentKey: "dk", userId: "u" });
    const data = { ...legacySettings, settings: { username: "alice" } };
    const result = migrateAuth(auth, data);

    expect(result.kind).toBe("already-migrated");
    const stripped = result.strippedData as Record<string, unknown>;
    expect(stripped.token).toBeUndefined();
    expect(stripped.deploymentKey).toBeUndefined();
    expect((stripped.settings as Record<string, unknown>).username).toBe("alice");
  });

  it("already migrated, no residue → already-migrated, no strip needed", () => {
    const ls = makeLocalStorage();
    const auth = new AuthStore(ls.load, ls.save);
    auth.save({ deviceId: "dev-1", token: "tok-1", serverUrl: "https://s", deploymentKey: "dk", userId: "u" });
    const data = { settings: { username: "alice" } };
    const result = migrateAuth(auth, data);
    expect(result.kind).toBe("already-migrated");
    expect(result.strippedData).toBeNull();
  });

  it("write-failed (verify read-back mismatch) → degraded, no data.json strip", () => {
    const ls = makeLocalStorage();
    ls.save.mockImplementation(() => {});
    const auth = new AuthStore(ls.load, ls.save);
    const data = { settings: { ...legacySettings } };
    const result = migrateAuth(auth, data);
    expect(result.kind).toBe("write-failed-degraded");
    expect(result.strippedData).toBeNull();
  });

  it("survives a plugin-folder wipe: data.json reset but localStorage intact", () => {
    const ls = makeLocalStorage();
    const auth = new AuthStore(ls.load, ls.save);
    // device had migrated previously
    auth.save({ deviceId: "dev-keep", token: "tok-keep", serverUrl: "https://s", deploymentKey: "dk", userId: "u" });

    // user deletes plugin folder → data.json is gone (empty object on next load)
    const result = migrateAuth(auth, {});

    // migration sees localStorage already has auth → already-migrated, no data loss
    expect(result.kind).toBe("already-migrated");
    // auth identity intact
    expect(auth.load()?.deviceId).toBe("dev-keep");
    expect(auth.load()?.token).toBe("tok-keep");
  });
});
