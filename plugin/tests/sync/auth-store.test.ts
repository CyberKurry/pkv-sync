import { describe, expect, it, vi } from "vitest";
import { AuthStore, type AuthData } from "../../src/sync/auth-store";

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
});
