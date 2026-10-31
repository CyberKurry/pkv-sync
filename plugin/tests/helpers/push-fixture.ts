import { createHash } from "node:crypto";
import { vi } from "vitest";
import { requestUrl } from "obsidian";
import { ApiClient } from "../../src/api/client";
import { SyncApi } from "../../src/api/sync-client";
import { SyncEngine, type IndexPersistence } from "../../src/sync/engine";
import type { LocalFileSnapshot, LocalIndex, PushChange } from "../../src/sync/types";
import type { VaultAdapter } from "../../src/sync/vault-adapter";

const DEFAULT_BODY_LIMIT = 2_097_152;

export interface WireCall {
  path: string;
  bytes: number;
  count: number;
  status: number;
  bodyHash: string;
  idempotencyKey: string | undefined;
}

export interface PushFixture {
  engine: SyncEngine;
  api: SyncApi;
  index: { value: LocalIndex };
  calls: WireCall[];
  setBodyLimit(bytes: number): void;
}

class Index implements IndexPersistence {
  value: LocalIndex = { lastSyncedCommit: null, files: {} };

  async loadIndex(): Promise<LocalIndex> {
    return this.value;
  }

  async updateIndex(
    updater: (index: LocalIndex) => LocalIndex | Promise<LocalIndex>
  ): Promise<void> {
    this.value = await updater(this.value);
  }
}

class Vault {
  configDir = ".obsidian";

  constructor(readonly files: LocalFileSnapshot[]) {}

  async scan(): Promise<LocalFileSnapshot[]> {
    return this.files.map(({ content: _content, bytes: _bytes, ...metadata }) => metadata);
  }

  async snapshot(path: string): Promise<LocalFileSnapshot> {
    const file = this.files.find((entry) => entry.path === path);
    if (!file) throw new Error(path);
    return file;
  }
}

export function textSnapshots(
  count: number,
  textBytes: number,
  includeBlob = false
): LocalFileSnapshot[] {
  const content = "a".repeat(textBytes);
  const hash = createHash("sha256").update(content).digest("hex");
  const files: LocalFileSnapshot[] = Array.from({ length: count }, (_, index) => ({
    kind: "text",
    path: `note-${index.toString().padStart(4, "0")}.md`,
    content,
    hash,
    size: textBytes
  }));
  if (includeBlob) {
    const bytes = new Uint8Array([1, 2, 3]);
    files.push({
      kind: "blob",
      path: "image.png",
      hash: createHash("sha256").update(bytes).digest("hex"),
      size: bytes.byteLength,
      bytes: bytes.buffer
    });
  }
  return files;
}

export function createPushFixture(files: LocalFileSnapshot[]): PushFixture {
  const calls: WireCall[] = [];
  let bodyLimit = DEFAULT_BODY_LIMIT;
  let commit = 0;
  vi.mocked(requestUrl).mockImplementation((request) => {
    const params = typeof request === "string" ? { url: request } : request;
    const path = new URL(params.url).pathname;
    const body = typeof params.body === "string" ? params.body : "";
    const bytes = new TextEncoder().encode(body).byteLength;
    const bodyHash = createHash("sha256").update(body).digest("hex");
    let status = 200;
    let data: unknown;
    if (path.endsWith("/push")) {
      const changes = (JSON.parse(body) as { changes: PushChange[] }).changes;
      status = bytes > bodyLimit ? 413 : 200;
      data =
        status === 413
          ? { error: { code: "http_413", message: "HTTP 413" } }
          : { new_commit: `c${++commit}`, files_changed: changes.length };
      calls.push({
        path,
        bytes,
        count: changes.length,
        status,
        bodyHash,
        idempotencyKey: params.headers?.["idempotency-key"]
      });
    } else if (path.endsWith("/upload/check")) {
      data = { missing: [] };
      calls.push({
        path,
        bytes,
        count: 0,
        status,
        bodyHash,
        idempotencyKey: params.headers?.["idempotency-key"]
      });
    } else if (path.endsWith("/pull")) {
      data = { from: null, to: null, added: [], modified: [], deleted: [] };
    } else {
      throw new Error(`Unexpected endpoint: ${path}`);
    }
    return Promise.resolve({
      status,
      headers: { "content-type": "application/json" },
      text: JSON.stringify(data),
      json: data,
      arrayBuffer: new ArrayBuffer(0)
    }) as unknown as ReturnType<typeof requestUrl>;
  });
  const index = new Index();
  const api = new SyncApi(
    new ApiClient({
      serverUrl: "https://issue-5.invalid",
      deploymentKey: "diagnostic",
      token: "diagnostic",
      pluginVersion: "1.7.1"
    })
  );
  const engine = new SyncEngine({
    vaultId: "v",
    deviceName: "issue-5-repro",
    textExtensions: new Set(["md"]),
    vault: new Vault(files) as unknown as VaultAdapter,
    api,
    index,
    vaultSettingsReader: async () => ({ extra_sync_globs: [] }),
    setStatus: vi.fn()
  });
  return {
    engine,
    api,
    index,
    calls,
    setBodyLimit(bytes: number) {
      bodyLimit = bytes;
    }
  };
}
