import { TFile, type Vault } from "obsidian";
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
    return this.vault.read(this.requireFile(path));
  }

  async readBinary(path: string): Promise<ArrayBuffer> {
    return this.vault.readBinary(this.requireFile(path));
  }

  async writeText(path: string, content: string): Promise<void> {
    const file = this.vault.getAbstractFileByPath(path);
    if (file instanceof TFile) await this.vault.modify(file, content);
    else {
      await this.ensureParentFolders(path);
      await this.vault.create(path, content);
    }
  }

  async writeBinary(path: string, bytes: ArrayBuffer): Promise<void> {
    const file = this.vault.getAbstractFileByPath(path);
    if (file instanceof TFile) await this.vault.modifyBinary(file, bytes);
    else {
      await this.ensureParentFolders(path);
      await this.vault.createBinary(path, bytes);
    }
  }

  async delete(path: string): Promise<void> {
    const file = this.vault.getAbstractFileByPath(path);
    if (file) await this.vault.delete(file);
  }

  exists(path: string): boolean {
    return this.vault.getAbstractFileByPath(path) instanceof TFile;
  }

  async snapshot(
    path: string,
    textExtensions: Set<string>
  ): Promise<LocalFileSnapshot> {
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
  if (path.startsWith(".obsidian/")) return false;
  if (path.startsWith(".trash/")) return false;
  const name = path.split("/").pop() ?? path;
  if (name.includes(".conflict-")) return false;
  return true;
}
