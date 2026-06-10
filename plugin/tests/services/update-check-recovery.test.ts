import { describe, expect, it } from "vitest";
import {
  recoverPendingUpdate,
  type PluginFileAdapter
} from "../../src/services/update-check";
import { sha256Text } from "../../src/sync/hash";

class MemoryAdapter implements PluginFileAdapter {
  files = new Map<string, string>();
  removed: string[] = [];
  writes: Array<{ path: string; data: string }> = [];

  async read(path: string): Promise<string> {
    const value = this.files.get(path);
    if (value === undefined) throw new Error(`missing ${path}`);
    return value;
  }

  async write(path: string, data: string): Promise<void> {
    this.writes.push({ path, data });
    this.files.set(path, data);
  }

  async remove(path: string): Promise<void> {
    this.removed.push(path);
    this.files.delete(path);
  }
}

const opts = (adapter: PluginFileAdapter) => ({
  adapter,
  configDir: ".obsidian",
  pluginId: "pkv-sync"
});

const pathFor = (fileName: string) =>
  `.obsidian/plugins/pkv-sync/${fileName}`;

describe("plugin update recovery", () => {
  it("promotes staged main.js when expected sha matches", async () => {
    const adapter = new MemoryAdapter();
    const staged = "console.log('new main')";
    adapter.files.set(pathFor(".main.js.new"), staged);
    adapter.files.set(pathFor(".main.js.sha256"), await sha256Text(staged));
    adapter.files.set(pathFor("main.js"), "old main");

    await recoverPendingUpdate(opts(adapter));

    expect(adapter.files.get(pathFor("main.js"))).toBe(staged);
    expect(adapter.files.has(pathFor(".main.js.new"))).toBe(false);
    expect(adapter.files.has(pathFor(".main.js.sha256"))).toBe(false);
  });

  it("discards staged main.js when expected sha mismatches", async () => {
    const adapter = new MemoryAdapter();
    adapter.files.set(pathFor(".main.js.new"), "tampered");
    adapter.files.set(pathFor(".main.js.sha256"), "0".repeat(64));
    adapter.files.set(pathFor("main.js"), "old main");

    await recoverPendingUpdate(opts(adapter));

    expect(adapter.files.get(pathFor("main.js"))).toBe("old main");
    expect(adapter.files.has(pathFor(".main.js.new"))).toBe(false);
    expect(adapter.files.has(pathFor(".main.js.sha256"))).toBe(false);
  });

  it("does nothing when no staged update files exist", async () => {
    const adapter = new MemoryAdapter();
    adapter.files.set(pathFor("main.js"), "current main");

    await recoverPendingUpdate(opts(adapter));

    expect(adapter.files.get(pathFor("main.js"))).toBe("current main");
    expect(adapter.writes).toEqual([]);
    expect(adapter.removed).toEqual([]);
  });
});
