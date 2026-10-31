import { writePluginSettingsPatch, writeSyncIndex } from "../plugin-data";
import type { LocalIndex } from "./types";

export interface MigrationSelection {
  selectedVaultId: string;
  selectedVaultName: string;
  lastSyncSuccessAt: number;
}

export function writeMigrationCompletion(
  raw: unknown,
  selection: MigrationSelection,
  targetScopeKey: string,
  index: LocalIndex
) {
  return writeSyncIndex(
    writePluginSettingsPatch(raw, selection),
    targetScopeKey,
    index
  );
}
