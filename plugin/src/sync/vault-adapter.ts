import { TFile, TFolder, type Vault } from "obsidian";
import { isConflictPath } from "./conflict-files";
import { sha256Bytes, sha256Text } from "./hash";
import { textByteLength } from "./text-encoding";
import type { LocalFileSnapshot, LocalIndex } from "./types";
import { extensionOf } from "../util";

const SCAN_SNAPSHOT_BATCH_SIZE = 8;

export interface VaultAdapter {
  listFiles(): TFile[];
  readText(path: string): Promise<string>;
  readBinary(path: string): Promise<ArrayBuffer>;
  writeText(path: string, content: string): Promise<void>;
  writeBinary(path: string, bytes: ArrayBuffer): Promise<void>;
  delete(path: string): Promise<void>;
  exists(path: string): boolean;
  snapshot(path: string, textExtensions: Set<string>): Promise<LocalFileSnapshot>;
  scan(
    textExtensions: Set<string>,
    previousIndex?: LocalIndex
  ): Promise<LocalFileSnapshot[]>;
}

export class ObsidianVaultAdapter implements VaultAdapter {
  constructor(private vault: Vault) {}

  listFiles(): TFile[] {
    return this.vault.getFiles();
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

  async delete(path: string): Promise<void> {
    const safePath = requireSafeVaultPath(path);
    const file = this.vault.getAbstractFileByPath(safePath);
    if (file) await this.vault.delete(file);
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
    const ext = extensionOf(path);
    if (textExtensions.has(ext)) {
      const content = await this.readText(path);
      return {
        path,
        hash: await sha256Text(content),
        size: textByteLength(content),
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
    previousIndex?: LocalIndex
  ): Promise<LocalFileSnapshot[]> {
    const files = this.listFiles().filter((file) => shouldSyncPath(file.path));
    const out: LocalFileSnapshot[] = [];
    const changedFiles: Array<{ index: number; path: string }> = [];
    for (const [index, file] of files.entries()) {
      const previous = previousIndex?.files[file.path];
      if (
        previous?.lastSyncedMtime === file.stat.mtime &&
        previous.size === file.stat.size
      ) {
        out[index] = {
          path: file.path,
          hash: previous.lastSyncedHash,
          size: file.stat.size,
          mtime: file.stat.mtime,
          kind: previous.kind
        };
        continue;
      }
      changedFiles.push({ index, path: file.path });
    }
    for (let i = 0; i < changedFiles.length; i += SCAN_SNAPSHOT_BATCH_SIZE) {
      const batch = changedFiles.slice(i, i + SCAN_SNAPSHOT_BATCH_SIZE);
      const results = await Promise.allSettled(
        batch.map(({ path }) => this.snapshot(path, textExtensions))
      );
      for (const [batchIndex, result] of results.entries()) {
        if (result.status === "rejected") throw result.reason;
        out[batch[batchIndex].index] = result.value;
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

export function shouldAcceptRemoteConflictPath(path: string): boolean {
  const normalized = normalizeVaultPath(path);
  return (
    normalized !== null &&
    isConflictPath(normalized) &&
    !hasProtectedSegment(normalized)
  );
}

export function normalizeSyncPath(path: string): string | null {
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
