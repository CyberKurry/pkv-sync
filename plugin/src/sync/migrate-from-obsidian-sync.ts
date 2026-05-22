import { TFile, TFolder } from "obsidian";

const COMMUNITY_PLUGINS_PATH = ".obsidian/community-plugins.json";
const SYNC_DIR_PATH = ".obsidian/sync";

export interface ObsidianSyncDetection {
  syncDirExists: boolean;
  syncPluginEnabled: boolean;
  likelyUsingSync: boolean;
}

export interface MigrationFile {
  path: string;
  size: number;
}

export interface MigrationScan {
  files: MigrationFile[];
  skippedCount: number;
  totalBytes: number;
}

interface MigrationVault {
  getFiles(): TFile[];
  getAbstractFileByPath(path: string): unknown;
  read(file: TFile): Promise<string>;
}

export async function detectObsidianSync(
  vault: MigrationVault
): Promise<ObsidianSyncDetection> {
  const syncDirExists =
    vault.getAbstractFileByPath(SYNC_DIR_PATH) instanceof TFolder;
  const syncPluginEnabled = await hasObsidianSyncCommunityPlugin(vault);

  return {
    syncDirExists,
    syncPluginEnabled,
    likelyUsingSync: syncDirExists || syncPluginEnabled
  };
}

export function scanVaultForMigration(vault: Pick<MigrationVault, "getFiles">): MigrationScan {
  const files: MigrationFile[] = [];
  let skippedCount = 0;
  let totalBytes = 0;

  for (const file of vault.getFiles()) {
    if (isMigrationExcluded(file.path)) {
      skippedCount++;
      continue;
    }

    const size = fileSize(file);
    files.push({ path: file.path, size });
    totalBytes += size;
  }

  return { files, skippedCount, totalBytes };
}

async function hasObsidianSyncCommunityPlugin(vault: MigrationVault): Promise<boolean> {
  const file = vault.getAbstractFileByPath(COMMUNITY_PLUGINS_PATH);
  if (!(file instanceof TFile)) return false;

  try {
    const plugins: unknown = JSON.parse(await vault.read(file));
    return Array.isArray(plugins) && plugins.includes("obsidian-sync");
  } catch {
    return false;
  }
}

function isMigrationExcluded(path: string): boolean {
  const normalized = path.replace(/\\/g, "/");
  const fileName = normalized.split("/").at(-1) ?? normalized;

  return (
    normalized === ".obsidian/workspace.json" ||
    normalized === ".obsidian/workspace-mobile.json" ||
    normalized === ".obsidian/workspaces.json" ||
    normalized === ".obsidian/cache" ||
    normalized.startsWith(".obsidian/cache/") ||
    normalized.startsWith(".obsidian/sync/") ||
    normalized.startsWith(".obsidian/plugins/pkv-sync/") ||
    normalized.startsWith(".trash/") ||
    normalized.endsWith(".lock") ||
    normalized.endsWith(".tmp") ||
    fileName === ".DS_Store" ||
    fileName === "Thumbs.db"
  );
}

function fileSize(file: TFile): number {
  return file.stat.size;
}
