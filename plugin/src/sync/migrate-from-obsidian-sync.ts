import { TFile, TFolder } from "obsidian";
import type { VaultSummary } from "../api/types";
import { sha256Bytes, sha256TextWithLength } from "./hash";
import { guessMime } from "./mime";
import type { LocalFileSnapshot, LocalIndex, PushChange, PushResponse, StateResponse } from "./types";
import { errorToMessage, isTextPath } from "../util";
import { HARD_EXCLUDE_GLOBS, createExcludeMatcher, isHiddenPath } from "./exclude";

const COMMUNITY_PLUGINS_PATH = ".obsidian/community-plugins.json";
const SYNC_DIR_PATH = ".obsidian/sync";
const DEFAULT_BATCH_SIZE = 50;

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
  readBinary(file: TFile): Promise<ArrayBuffer>;
}

export interface MigrationApi {
  createVault(name: string): Promise<VaultSummary>;
  state(vaultId: string, headSince: string | null): Promise<StateResponse>;
  uploadCheck(vaultId: string, hashes: string[]): Promise<{ missing: string[] }>;
  uploadBlob(vaultId: string, hash: string, bytes: ArrayBuffer): Promise<void>;
  push(
    vaultId: string,
    ifMatch: string | null,
    changes: PushChange[],
    deviceName: string
  ): Promise<PushResponse>;
}

export type MigrationStage =
  | "scanning"
  | "creating_vault"
  | "uploading_blobs"
  | "pushing"
  | "complete";

export interface MigrationProgress {
  stage: MigrationStage;
  totalFiles: number;
  processedFiles: number;
  pushedFiles: number;
  skippedCount: number;
  totalBytes: number;
  uploadedBlobs: number;
  totalBlobs: number;
  currentBatch: number;
  totalBatches: number;
}

export interface MigrationOptions {
  vault: MigrationVault;
  api: MigrationApi;
  vaultName: string;
  deviceName: string;
  textExtensions: Set<string>;
  batchSize?: number;
  onProgress?: (progress: MigrationProgress) => void;
}

export interface MigrationResult {
  vaultId: string;
  vaultName: string;
  scannedFiles: number;
  pushedFiles: number;
  skippedCount: number;
  totalBytes: number;
  batches: number;
  lastCommit: string | null;
  index: LocalIndex;
}

export class MigrationError extends Error {
  constructor(
    message: string,
    public readonly batch: number,
    public readonly totalBatches: number,
    public readonly cause: unknown
  ) {
    super(message);
    this.name = "MigrationError";
  }
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

export async function migrateToPkv(options: MigrationOptions): Promise<MigrationResult> {
  const scan = scanVaultForMigration(options.vault);
  const batchSize = normalizeBatchSize(options.batchSize);
  const totalBatches = Math.ceil(scan.files.length / batchSize);
  // Blob classification is purely path-based, so the total blob count is
  // known before any content is read.
  const totalBlobs = scan.files.filter(
    (file) => !isTextPath(file.path, options.textExtensions)
  ).length;
  const progressBase = {
    totalFiles: scan.files.length,
    processedFiles: 0,
    pushedFiles: 0,
    skippedCount: scan.skippedCount,
    totalBytes: scan.totalBytes,
    uploadedBlobs: 0,
    totalBlobs,
    currentBatch: 0,
    totalBatches
  };

  options.onProgress?.({ stage: "scanning", ...progressBase });
  options.onProgress?.({ stage: "creating_vault", ...progressBase });
  const created = await options.api.createVault(options.vaultName.trim());
  const initialState = await options.api.state(created.id, null);
  let head = initialState.current_head;

  const withBlobTotals = { ...progressBase, totalBlobs };
  if (totalBlobs === 0) {
    options.onProgress?.({ stage: "uploading_blobs", ...withBlobTotals });
  }

  // Snapshot, upload and push per batch, releasing each batch's payloads
  // before reading the next, so peak memory is one batch instead of the whole
  // vault. The index needs only metadata, accumulated here.
  const indexMetadata: LocalFileSnapshot[] = [];
  let uploadedBlobs = 0;
  let pushedFiles = 0;

  for (let offset = 0; offset < scan.files.length; offset += batchSize) {
    const batchIndex = Math.floor(offset / batchSize) + 1;
    const batchFiles = scan.files.slice(offset, offset + batchSize);
    const batchSnapshots: LocalFileSnapshot[] = [];
    for (const file of batchFiles) {
      const snapshot = await snapshotMigrationFile(
        options.vault,
        file.path,
        options.textExtensions
      );
      indexMetadata.push({
        path: snapshot.path,
        hash: snapshot.hash,
        size: snapshot.size,
        kind: snapshot.kind
      });
      batchSnapshots.push(snapshot);
    }

    const batchBlobs = batchSnapshots.filter((file) => file.kind === "blob");
    if (batchBlobs.length > 0) {
      const hashes = batchBlobs.map((file) => file.hash);
      const missing = await options.api.uploadCheck(created.id, hashes);
      const missingSet = new Set(missing.missing);
      for (const file of batchBlobs) {
        if (!missingSet.has(file.hash)) continue;
        if (!file.bytes) throw new Error(`Missing bytes for blob ${file.path}`);
        await options.api.uploadBlob(created.id, file.hash, file.bytes);
        uploadedBlobs += 1;
        options.onProgress?.({
          stage: "uploading_blobs",
          ...withBlobTotals,
          uploadedBlobs,
          currentBatch: batchIndex
        });
      }
    }

    const changes = batchSnapshots.map(snapshotToPushChange);
    try {
      const response = await options.api.push(
        created.id,
        head,
        changes,
        options.deviceName
      );
      head = response.new_commit;
    } catch (error) {
      throw new MigrationError(
        `Migration failed while pushing batch ${batchIndex} of ${totalBatches}: ${errorMessage(error)}`,
        batchIndex,
        totalBatches,
        error
      );
    }
    pushedFiles += batchSnapshots.length;
    options.onProgress?.({
      stage: "pushing",
      ...withBlobTotals,
      processedFiles: pushedFiles,
      pushedFiles,
      uploadedBlobs,
      currentBatch: batchIndex
    });
  }

  const index = buildMigrationIndex(indexMetadata, head);
  const result: MigrationResult = {
    vaultId: created.id,
    vaultName: created.name,
    scannedFiles: scan.files.length,
    pushedFiles,
    skippedCount: scan.skippedCount,
    totalBytes: scan.totalBytes,
    batches: totalBatches,
    lastCommit: head,
    index
  };

  options.onProgress?.({
    stage: "complete",
    ...withBlobTotals,
    processedFiles: pushedFiles,
    pushedFiles,
    uploadedBlobs,
    currentBatch: totalBatches
  });

  return result;
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

const MIGRATION_EXTRA_GLOBS = [
  ".obsidian/cache",
  ".obsidian/sync/**",
  ".obsidian/plugins/pkv-sync/**",
  ".obsidian/plugins/**",
  "**/.env*",
  "**/.DS_Store",
  "**/Thumbs.db"
];

const MIGRATION_OBSIDIAN_CORE_GLOBS = [
  ".obsidian/app.json",
  ".obsidian/appearance.json",
  ".obsidian/core-plugins.json",
  ".obsidian/core-plugins-migration.json",
  ".obsidian/hotkeys.json",
  ".obsidian/graph.json",
  ".obsidian/bookmarks.json",
  ".obsidian/community-plugins.json"
];

const migrationExcludeMatcher = createExcludeMatcher([
  ...HARD_EXCLUDE_GLOBS,
  ...MIGRATION_EXTRA_GLOBS
]);

const migrationObsidianCoreMatcher = createExcludeMatcher(
  MIGRATION_OBSIDIAN_CORE_GLOBS
);

function isMigrationExcluded(path: string): boolean {
  const normalized = path.replace(/\\/g, "/");
  if (migrationExcludeMatcher(normalized)) return true;
  if (migrationObsidianCoreMatcher(normalized)) return false;
  return isHiddenPath(normalized);
}

function fileSize(file: TFile): number {
  return file.stat.size;
}

async function snapshotMigrationFile(
  vault: MigrationVault,
  path: string,
  textExtensions: Set<string>
): Promise<LocalFileSnapshot> {
  const file = vault.getAbstractFileByPath(path);
  if (!(file instanceof TFile)) throw new Error(`File not found: ${path}`);

  if (isTextPath(path, textExtensions)) {
    const content = await vault.read(file);
    const { hash, byteLength } = await sha256TextWithLength(content);
    return {
      path,
      hash,
      size: byteLength,
      kind: "text",
      content
    };
  }

  const bytes = await vault.readBinary(file);
  return {
    path,
    hash: await sha256Bytes(bytes),
    size: bytes.byteLength,
    kind: "blob",
    bytes
  };
}

function snapshotToPushChange(file: LocalFileSnapshot): PushChange {
  if (file.kind === "text") {
    return { kind: "text", path: file.path, content: file.content ?? "" };
  }
  return {
    kind: "blob",
    path: file.path,
    blob_hash: file.hash,
    size: file.size,
    mime: guessMime(file.path)
  };
}

function buildMigrationIndex(files: LocalFileSnapshot[], commit: string | null): LocalIndex {
  const entries: LocalIndex["files"] = {};
  const syncedAt = Date.now();
  for (const file of files) {
    entries[file.path] = {
      lastSyncedHash: file.hash,
      lastSyncedAt: syncedAt,
      kind: file.kind,
      size: file.size
    };
  }
  return { lastSyncedCommit: commit, files: entries };
}

function normalizeBatchSize(value: number | undefined): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return DEFAULT_BATCH_SIZE;
  return Math.max(1, Math.floor(value));
}

const errorMessage = errorToMessage;
