import { TFile, type Vault } from "obsidian";
import { isConflictPath } from "./conflict-files";
import { sha256Bytes, sha256Text } from "./hash";
import type { LocalFileSnapshot } from "./types";

export interface VaultAdapter {
  listFiles(): TFile[];
  readText(path: string): Promise<string>;
  readBinary(path: string): Promise<ArrayBuffer>;
  writeText(path: string, content: string): Promise<void>;
  writeBinary(path: string, bytes: ArrayBuffer): Promise<void>;
  delete(path: string): Promise<void>;
  exists(path: string): boolean;
  snapshot(path: string, textExtensions: Set<string>): Promise<LocalFileSnapshot>;
  scan(textExtensions: Set<string>): Promise<LocalFileSnapshot[]>;
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
    const ext = path.includes(".") ? path.split(".").pop()!.toLowerCase() : "";
    if (textExtensions.has(ext)) {
      const content = await this.readText(path);
      return {
        path,
        hash: await sha256Text(content),
        size: new TextEncoder().encode(content).byteLength,
        kind: "text",
        content
      };
    }
    const bytes = await this.readBinary(path);
    return {
      path,
      hash: await sha256Bytes(bytes),
      size: bytes.byteLength,
      kind: "blob",
      bytes
    };
  }

  async scan(textExtensions: Set<string>): Promise<LocalFileSnapshot[]> {
    const files = this.listFiles().filter((file) => shouldSyncPath(file.path));
    const out: LocalFileSnapshot[] = [];
    for (const file of files) {
      out.push(await this.snapshot(file.path, textExtensions));
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
      if (!this.vault.getFolderByPath(current)) {
        await this.vault.createFolder(current);
      }
    }
  }
}

export function shouldSyncPath(path: string): boolean {
  return normalizeSyncPath(path) !== null;
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
    if (!isSafePathShape(current) || hasProtectedRoot(current)) return true;
  }
  return false;
}

function hasProtectedRoot(path: string): boolean {
  const firstSegment = path.split("/", 1)[0].toLowerCase();
  return firstSegment === ".obsidian" || firstSegment === ".trash" || firstSegment === ".git";
}
