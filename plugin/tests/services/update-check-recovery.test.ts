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

  it("restores main.js from .bak when the staged copy is broken and the target fails verification (R17-M1)", async () => {
    // The kill can land anywhere in write temp → sha → backup → target →
    // verify. A target truncated mid-write never runs the verify branch,
    // and the old recovery only consulted .new + .sha256: with a broken
    // staged copy it left the truncated main.js in place — the plugin
    // could not load, so the recovery code inside it never ran again.
    // The .bak the write sequence maintains holds the previous GOOD
    // content; it is the only intact artifact left.
    const adapter = new MemoryAdapter();
    adapter.files.set(pathFor(".main.js.new"), "tampered");
    adapter.files.set(pathFor(".main.js.sha256"), "0".repeat(64));
    adapter.files.set(pathFor("main.js"), "truncated main");
    adapter.files.set(pathFor(".main.js.bak"), "old good main");

    await recoverPendingUpdate(opts(adapter));

    expect(adapter.files.get(pathFor("main.js"))).toBe("old good main");
  });

  it("restores main.js from .bak when the target is missing (R17-M1)", async () => {
    const adapter = new MemoryAdapter();
    adapter.files.set(pathFor(".main.js.new"), "tampered");
    adapter.files.set(pathFor(".main.js.sha256"), "0".repeat(64));
    adapter.files.set(pathFor(".main.js.bak"), "old good main");

    await recoverPendingUpdate(opts(adapter));

    expect(adapter.files.get(pathFor("main.js"))).toBe("old good main");
  });

  it("recovers manifest.json and styles.css before main.js (R17-M1)", async () => {
    // main.js is the entry point: if recovery dies partway, the assets it
    // depends on must already be consistent, so main.js is restored last.
    const adapter = new MemoryAdapter();
    for (const name of ["main.js", "manifest.json", "styles.css"]) {
      adapter.files.set(pathFor(`.${name}.new`), `tampered ${name}`);
      adapter.files.set(pathFor(`.${name}.sha256`), "0".repeat(64));
      adapter.files.set(pathFor(name), `truncated ${name}`);
      adapter.files.set(pathFor(`.${name}.bak`), `good ${name}`);
    }

    await recoverPendingUpdate(opts(adapter));

    const restoredOrder = adapter.writes
      .filter((write) => !write.path.includes("/."))
      .map((write) => write.path.slice(pathFor("").length));
    expect(restoredOrder).toEqual(["manifest.json", "styles.css", "main.js"]);
  });
});
