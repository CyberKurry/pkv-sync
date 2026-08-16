import { TFile, TFolder, type Vault } from "obsidian";
import { isConflictPath } from "./conflict-files";
import { sha256Bytes, sha256TextWithLength } from "./hash";
import type { LocalFileSnapshot, LocalIndex } from "./types";
import { isTextPath } from "../util";

const SCAN_SNAPSHOT_BATCH_SIZE = 8;

export interface VaultAdapter {
  listFiles(): TFile[];
  readText(path: string): Promise<string>;
  readBinary(path: string): Promise<ArrayBuffer>;
  writeText(path: string, content: string): Promise<void>;
  writeBinary(path: string, bytes: ArrayBuffer): Promise<void>;
  trash(path: string): Promise<void>;
  exists(path: string): boolean;
  snapshot(path: string, textExtensions: Set<string>): Promise<LocalFileSnapshot>;
  scan(
    textExtensions: Set<string>,
    previousIndex?: LocalIndex,
    options?: VaultScanOptions
  ): Promise<LocalFileSnapshot[]>;
}

export interface VaultScanOptions {
  retainPayload?: boolean;
}

export class ObsidianVaultAdapter implements VaultAdapter {
  constructor(private vault: Vault) {}

  listFiles(): TFile[] {
    const files = this.vault.getFiles();
    // Obsidian's vault.getFiles() reads the file cache, which does not index
    // the .obsidian directory. Walk that folder subtree directly so allowlisted
    // .obsidian paths (themes, snippets, plugin configs) are discoverable for
    // sync. Paths already returned by getFiles() are deduplicated.
    const known = new Set(files.map((file) => file.path));
    const obsidian = this.vault.getAbstractFileByPath(".obsidian");
    const extra: TFile[] = [];
    if (obsidian instanceof TFolder) {
      collectFilesRecursive(obsidian, known, extra);
    }
    return extra.length === 0 ? files : files.concat(extra);
  }

  async readText(path: string): Promise<string> {
    return this.vault.read(this.requireFile(requireSafeVaultPath(path)));
  }

  async readBinary(path: string): Promise<ArrayBuffer> {
    return this.vault.readBinary(this.requireFile(requireSafeVaultPath(path)));
  }

  async writeText(path: string, content: string): Promise<void> {
    const safePath = requireSafeVaultPath(path);
    const file = this.vault.getAbstractFileByPath(safePath);
    if (file instanceof TFile) await this.vault.modify(file, content);
    else {
      await this.ensureParentFolders(safePath);
      await this.vault.create(safePath, content);
    }
  }

  async writeBinary(path: string, bytes: ArrayBuffer): Promise<void> {
    const safePath = requireSafeVaultPath(path);
    const file = this.vault.getAbstractFileByPath(safePath);
    if (file instanceof TFile) await this.vault.modifyBinary(file, bytes);
    else {
      await this.ensureParentFolders(safePath);
      await this.vault.createBinary(safePath, bytes);
    }
  }

  async trash(path: string): Promise<void> {
    const safePath = requireSafeVaultPath(path);
    const file = this.vault.getAbstractFileByPath(safePath);
    if (file) await this.vault.trash(file, true);
  }

  exists(path: string): boolean {
    const safePath = normalizeVaultPath(path);
    return safePath !== null && this.vault.getAbstractFileByPath(safePath) instanceof TFile;
  }

  async snapshot(
    path: string,
    textExtensions: Set<string>
  ): Promise<LocalFileSnapshot> {
    path = requireSafeVaultPath(path);
    const file = this.requireFile(path);
    if (isTextPath(path, textExtensions)) {
      const content = await this.readText(path);
      const { hash, byteLength } = await sha256TextWithLength(content);
      return {
        path,
        hash,
        size: byteLength,
        mtime: file.stat.mtime,
        kind: "text",
        content
      };
    }
    const bytes = await this.readBinary(path);
    return {
      path,
      hash: await sha256Bytes(bytes),
      size: bytes.byteLength,
      mtime: file.stat.mtime,
      kind: "blob",
      bytes
    };
  }

  async scan(
    textExtensions: Set<string>,
    previousIndex?: LocalIndex,
    options?: VaultScanOptions
  ): Promise<LocalFileSnapshot[]> {
    const files = this.listFiles().filter((file) => shouldSyncPath(file.path));
    const out: LocalFileSnapshot[] = [];
    const scanFiles = files.map((file, index) => ({ index, path: file.path }));
    for (let i = 0; i < scanFiles.length; i += SCAN_SNAPSHOT_BATCH_SIZE) {
      const batch = scanFiles.slice(i, i + SCAN_SNAPSHOT_BATCH_SIZE);
      const results = await Promise.allSettled(
        batch.map(({ path }) => this.snapshot(path, textExtensions))
      );
      for (const [batchIndex, result] of results.entries()) {
        if (result.status === "rejected") throw result.reason;
        const snapshot = result.value;
        // The initial scan can contain thousands of changed files. Let the
        // caller retain only metadata and hydrate each bounded push batch
        // immediately before upload instead of keeping every payload in RAM.
        if (
          options?.retainPayload === false ||
          previousIndex?.files[snapshot.path]?.lastSyncedHash === snapshot.hash
        ) {
          delete snapshot.content;
          delete snapshot.bytes;
        }
        out[batch[batchIndex].index] = snapshot;
      }
    }
    return out;
  }

  private requireFile(path: string): TFile {
    const file = this.vault.getAbstractFileByPath(path);
    if (!(file instanceof TFile)) throw new Error(`File not found: ${path}`);
    return file;
  }

  private async ensureParentFolders(path: string): Promise<void> {
    const slash = path.lastIndexOf("/");
    if (slash < 0) return;
    const parent = path.slice(0, slash);
    const parts = parent.split("/");
    let current = "";
    for (const part of parts) {
      current = current ? `${current}/${part}` : part;
      if (!(this.vault.getAbstractFileByPath(current) instanceof TFolder)) {
        await this.vault.createFolder(current);
      }
    }
  }
}

export function shouldSyncPath(path: string): boolean {
  return normalizeSyncPath(path) !== null;
}

function collectFilesRecursive(folder: TFolder, known: Set<string>, out: TFile[]): void {
  for (const child of folder.children) {
    if (child instanceof TFolder) {
      collectFilesRecursive(child, known, out);
    } else if (child instanceof TFile && !known.has(child.path)) {
      known.add(child.path);
      out.push(child);
    }
  }
}

export function shouldAcceptRemoteConflictPath(path: string): boolean {
  const normalized = normalizeVaultPath(path);
  return (
    normalized !== null &&
    isConflictPath(normalized) &&
    !hasProtectedSegment(normalized)
  );
}

function normalizeSyncPath(path: string): string | null {
  const normalized = normalizeVaultPath(path);
  if (normalized === null) return null;
  if (isConflictPath(normalized)) return null;
  return normalized;
}

function requireSafeVaultPath(path: string): string {
  const normalized = normalizeVaultPath(path);
  if (normalized === null) throw new Error(`Unsafe sync path: ${path}`);
  return normalized;
}

function normalizeVaultPath(path: string): string | null {
  const normalized = normalizeSeparators(path);
  if (!isSafePathShape(normalized)) return null;
  if (hasUnsafeDecodedShape(normalized)) return null;
  if (hasProtectedRoot(normalized)) return null;
  return normalized;
}

function normalizeSeparators(path: string): string {
  return path.replace(/\\/g, "/");
}

function isSafePathShape(path: string): boolean {
  if (path.length === 0) return false;
  if (path.includes("\0")) return false;
  if (path.startsWith("/") || path.startsWith("//")) return false;
  if (/^[A-Za-z]:\//.test(path)) return false;
  const parts = path.split("/");
  if (parts.some((part) => part === "" || part === "." || part === "..")) {
    return false;
  }
  return true;
}

function hasUnsafeDecodedShape(path: string): boolean {
  let current = path;
  for (let i = 0; i < 4; i++) {
    let decoded: string;
    try {
      decoded = decodeURIComponent(current);
    } catch {
      return false;
    }
    if (decoded === current) return false;
    current = normalizeSeparators(decoded);
    if (!isSafePathShape(current) || hasProtectedRoot(current) || startsWithDotRoot(current)) {
      return true;
    }
  }
  return false;
}

function hasProtectedRoot(path: string): boolean {
  const firstSegment = path.split("/", 1)[0].toLowerCase();
  return firstSegment === ".trash" || firstSegment === ".git";
}

function hasProtectedSegment(path: string): boolean {
  return path
    .split("/")
    .some((segment) => segment.toLowerCase() === ".trash" || segment.toLowerCase() === ".git");
}

function startsWithDotRoot(path: string): boolean {
  return path.split("/", 1)[0].startsWith(".");
}
