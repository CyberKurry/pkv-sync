import { requestUrl } from "obsidian";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ApiClient } from "../../src/api/client";
import {
  UpdateCheckService,
  compareVersions,
  type PluginFileAdapter
} from "../../src/services/update-check";
import { sha256Text } from "../../src/sync/hash";

const requestUrlMock = vi.mocked(requestUrl);

class MemoryAdapter implements PluginFileAdapter {
  writes: Array<{ path: string; data: string }> = [];
  files = new Map<string, string>();
  removed: string[] = [];
  directories = new Set<string>();

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

  async mkdir(path: string): Promise<void> {
    this.directories.add(path);
  }
}

class DirectoryRequiredAdapter extends MemoryAdapter {
  async write(path: string, data: string): Promise<void> {
    const parent = path.split("/").slice(0, -1).join("/");
    if (parent && !this.directories.has(parent)) {
      throw new Error(`ENOENT: no such file or directory, open '${path}'`);
    }
    await super.write(path, data);
  }
}

class CorruptingTargetAdapter extends MemoryAdapter {
  corruptNextMainWrite = true;

  async write(path: string, data: string): Promise<void> {
    if (
      path === ".obsidian/plugins/pkv-sync/main.js" &&
      this.corruptNextMainWrite
    ) {
      this.corruptNextMainWrite = false;
      await super.write(path, `${data}\n// corrupted`);
      return;
    }
    await super.write(path, data);
  }
}

function responseJson(value: unknown): {
  status: number;
  headers: Record<string, string>;
  text: string;
  arrayBuffer: ArrayBuffer;
  json: unknown;
} {
  const text = JSON.stringify(value);
  return {
    status: 200,
    headers: { "content-type": "application/json" },
    text,
    arrayBuffer: new ArrayBuffer(0),
    json: value
  };
}

function responseText(text: string): {
  status: number;
  headers: Record<string, string>;
  text: string;
  arrayBuffer: ArrayBuffer;
  json: null;
} {
  return {
    status: 200,
    headers: { "content-type": "text/plain" },
    text,
    arrayBuffer: new TextEncoder().encode(text).buffer,
    json: null
  };
}

function service(
  adapter = new MemoryAdapter(),
  options: Partial<ConstructorParameters<typeof UpdateCheckService>[0]> = {}
): UpdateCheckService {
  const api = new ApiClient({
    serverUrl: "https://sync.example.com",
    deploymentKey: "k_abc",
    token: "tok",
    pluginVersion: "0.8.0"
  });
  return new UpdateCheckService({
    api,
    adapter,
    configDir: ".obsidian",
    currentVersion: "0.8.0",
    ...options
  });
}

describe("plugin update check", () => {
  beforeEach(() => {
    requestUrlMock.mockReset();
  });

  it("compares numeric semver segments", () => {
    expect(compareVersions("0.10.0", "0.9.9")).toBeGreaterThan(0);
    expect(compareVersions("0.8.0", "0.8.0")).toBe(0);
    expect(compareVersions("0.8.0-rc.1", "0.8.0")).toBeLessThan(0);
  });

  it("prefers the server bundled plugin manifest", async () => {
    requestUrlMock.mockResolvedValueOnce(
      responseJson({
        version: "0.8.1",
        main_js_url: "https://sync.example.com/api/plugin-assets/main.js",
        main_js_sha256: "a".repeat(64),
        manifest_json_url: "https://sync.example.com/api/plugin-assets/manifest.json",
        manifest_json_sha256: "b".repeat(64),
        styles_css_url: null,
        styles_css_sha256: null
      })
    );

    const update = await service().checkOnce("server");

    expect(update?.version).toBe("0.8.1");
    expect(update?.source).toBe("server");
    expect(requestUrlMock).toHaveBeenCalledTimes(1);
    expect(requestUrlMock).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "https://sync.example.com/api/plugin-manifest"
      })
    );
  });

  it("falls back to GitHub releases when server manifest is unavailable", async () => {
    const main = "console.log('0.8.1')";
    const manifest = '{"version":"0.8.1"}';
    requestUrlMock
      .mockResolvedValueOnce({
        status: 404,
        headers: {},
        text: "missing",
        arrayBuffer: new ArrayBuffer(0),
        json: null
      })
      .mockResolvedValueOnce(
        responseJson({
          tag_name: "v0.8.1",
          html_url: "https://github.com/cyberkurry/pkv-sync/releases/tag/v0.8.1",
          body: `${await sha256Text(main)}  main.js\n${await sha256Text(manifest)}  manifest.json`,
          assets: [
            {
              name: "main.js",
              browser_download_url:
                "https://github.com/cyberkurry/pkv-sync/releases/download/v0.8.1/main.js"
            },
            {
              name: "manifest.json",
              browser_download_url:
                "https://github.com/cyberkurry/pkv-sync/releases/download/v0.8.1/manifest.json"
            }
          ]
        })
      );

    const update = await service().checkOnce("server");

    expect(update).toMatchObject({
      version: "0.8.1",
      source: "github",
      releaseNotesUrl:
        "https://github.com/cyberkurry/pkv-sync/releases/tag/v0.8.1"
    });
  });

  it("rejects sha256 mismatches before writing plugin files", async () => {
    const adapter = new MemoryAdapter();
    requestUrlMock.mockResolvedValueOnce(responseText("changed main"));
    const update = {
      version: "0.8.1",
      source: "server" as const,
      releaseNotesUrl: "https://example.com/release",
      mainJsUrl: "https://example.com/main.js",
      mainJsSha256: "0".repeat(64),
      manifestJsonUrl: "https://example.com/manifest.json",
      manifestJsonSha256: null,
      stylesCssUrl: null,
      stylesCssSha256: null
    };

    await expect(service(adapter).applyUpdate(update)).rejects.toThrow(
      /sha256/i
    );

    expect(adapter.writes).toEqual([]);
  });

  it("writes verified files through temporary and backup paths", async () => {
    const adapter = new MemoryAdapter();
    adapter.files.set(".obsidian/plugins/pkv-sync/main.js", "old");
    adapter.files.set(".obsidian/plugins/pkv-sync/manifest.json", '{"version":"0.8.0"}');
    const main = "console.log('0.8.1')";
    const manifest = '{"version":"0.8.1"}';
    requestUrlMock
      .mockResolvedValueOnce(responseText(main))
      .mockResolvedValueOnce(responseText(manifest));
    const update = {
      version: "0.8.1",
      source: "server" as const,
      releaseNotesUrl: "https://example.com/release",
      mainJsUrl: "https://example.com/main.js",
      mainJsSha256: await sha256Text(main),
      manifestJsonUrl: "https://example.com/manifest.json",
      manifestJsonSha256: await sha256Text(manifest),
      stylesCssUrl: null,
      stylesCssSha256: null
    };

    await service(adapter).applyUpdate(update);

    expect(adapter.writes.map((write) => write.path)).toEqual([
      ".obsidian/plugins/pkv-sync/.main.js.new",
      ".obsidian/plugins/pkv-sync/.main.js.sha256",
      ".obsidian/plugins/pkv-sync/.main.js.bak",
      ".obsidian/plugins/pkv-sync/main.js",
      ".obsidian/plugins/pkv-sync/.manifest.json.new",
      ".obsidian/plugins/pkv-sync/.manifest.json.sha256",
      ".obsidian/plugins/pkv-sync/.manifest.json.bak",
      ".obsidian/plugins/pkv-sync/manifest.json"
    ]);
    expect(adapter.files.get(".obsidian/plugins/pkv-sync/main.js")).toBe(main);
    expect(adapter.removed).toContain(".obsidian/plugins/pkv-sync/.main.js.new");
  });

  it("creates the manifest plugin directory before staging update files", async () => {
    const adapter = new DirectoryRequiredAdapter();
    const main = "console.log('0.8.1')";
    const manifest = '{"version":"0.8.1"}';
    requestUrlMock
      .mockResolvedValueOnce(responseText(main))
      .mockResolvedValueOnce(responseText(manifest));
    const update = {
      version: "0.8.1",
      source: "server" as const,
      releaseNotesUrl: "https://example.com/release",
      mainJsUrl: "https://example.com/main.js",
      mainJsSha256: await sha256Text(main),
      manifestJsonUrl: "https://example.com/manifest.json",
      manifestJsonSha256: await sha256Text(manifest),
      stylesCssUrl: null,
      stylesCssSha256: null
    };

    await service(adapter, { pluginDir: ".obsidian/plugins/custom-folder" }).applyUpdate(update);

    expect(adapter.directories.has(".obsidian/plugins/custom-folder")).toBe(true);
    expect(adapter.files.get(".obsidian/plugins/custom-folder/main.js")).toBe(main);
    expect(adapter.writes[0]?.path).toBe(
      ".obsidian/plugins/custom-folder/.main.js.new"
    );
  });

  it("rolls back from backup when post-write verification fails", async () => {
    const adapter = new CorruptingTargetAdapter();
    adapter.files.set(".obsidian/plugins/pkv-sync/main.js", "old");
    const main = "console.log('0.8.1')";
    const manifest = '{"version":"0.8.1"}';
    requestUrlMock
      .mockResolvedValueOnce(responseText(main))
      .mockResolvedValueOnce(responseText(manifest));
    const update = {
      version: "0.8.1",
      source: "server" as const,
      releaseNotesUrl: "https://example.com/release",
      mainJsUrl: "https://example.com/main.js",
      mainJsSha256: await sha256Text(main),
      manifestJsonUrl: "https://example.com/manifest.json",
      manifestJsonSha256: await sha256Text(manifest),
      stylesCssUrl: null,
      stylesCssSha256: null
    };

    await expect(service(adapter).applyUpdate(update)).rejects.toThrow(
      /post-write sha256 mismatch/i
    );

    expect(adapter.files.get(".obsidian/plugins/pkv-sync/main.js")).toBe("old");
    expect(adapter.removed).toEqual(
      expect.arrayContaining([
        ".obsidian/plugins/pkv-sync/.main.js.new",
        ".obsidian/plugins/pkv-sync/.main.js.sha256"
      ])
    );
  });

  it("uses a bounded request timeout for public update downloads", async () => {
    const adapter = new MemoryAdapter();
    const main = "console.log('0.8.1')";
    const manifest = '{"version":"0.8.1"}';
    requestUrlMock
      .mockResolvedValueOnce(responseText(main))
      .mockResolvedValueOnce(responseText(manifest));
    const update = {
      version: "0.8.1",
      source: "github" as const,
      releaseNotesUrl: "https://example.com/release",
      mainJsUrl: "https://example.com/main.js",
      mainJsSha256: await sha256Text(main),
      manifestJsonUrl: "https://example.com/manifest.json",
      manifestJsonSha256: await sha256Text(manifest),
      stylesCssUrl: null,
      stylesCssSha256: null
    };

    await service(adapter).applyUpdate(update);

    expect(requestUrlMock).toHaveBeenCalledWith(
      expect.objectContaining({ requestTimeout: 30000 })
    );
  });

  it("downloads same-server assets with plugin auth headers", async () => {
    const adapter = new MemoryAdapter();
    const main = "console.log('server bundle')";
    const manifest = '{"version":"0.8.1"}';
    requestUrlMock
      .mockResolvedValueOnce({
        status: 200,
        headers: { "content-type": "application/javascript" },
        text: main,
        arrayBuffer: new TextEncoder().encode(main).buffer,
        json: null
      })
      .mockResolvedValueOnce({
        status: 200,
        headers: { "content-type": "application/json" },
        text: manifest,
        arrayBuffer: new TextEncoder().encode(manifest).buffer,
        json: JSON.parse(manifest)
      });
    const update = {
      version: "0.8.1",
      source: "server" as const,
      releaseNotesUrl: "https://example.com/release",
      mainJsUrl: "https://sync.example.com/api/plugin-assets/main.js",
      mainJsSha256: await sha256Text(main),
      manifestJsonUrl: "https://sync.example.com/api/plugin-assets/manifest.json",
      manifestJsonSha256: await sha256Text(manifest),
      stylesCssUrl: null,
      stylesCssSha256: null
    };

    await service(adapter).applyUpdate(update);

    expect(requestUrlMock).toHaveBeenCalledWith(
      expect.objectContaining({
        url: "https://sync.example.com/api/plugin-assets/main.js",
        headers: expect.objectContaining({
          Authorization: "Bearer tok",
          "X-PKVSync-Deployment-Key": "k_abc"
        })
      })
    );
    expect(adapter.files.get(".obsidian/plugins/pkv-sync/manifest.json")).toBe(
      manifest
    );
  });

});
