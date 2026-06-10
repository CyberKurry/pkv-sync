import type { Vault } from "obsidian";
import type { HistoryApi } from "../api/history-client";
import { errorToMessage } from "../util";
import { textByteLength } from "./text-encoding";
import { ObsidianVaultAdapter } from "./vault-adapter";

export interface RestoreContext {
  vault: Vault;
  api: HistoryApi;
  vaultId: string;
  path: string;
  atCommit: string;
  isBinary: boolean;
}

export type RestoreOutcome =
  | { ok: true; kind: "modified" | "created"; bytes: number }
  | {
      ok: false;
      reason: "deleted_at_commit" | "fetch_failed" | "write_failed";
      detail?: string;
    };

export async function restoreFileToCommit(
  ctx: RestoreContext
): Promise<RestoreOutcome> {
  let historical;
  try {
    historical = await ctx.api.readFileAt(ctx.vaultId, ctx.path, ctx.atCommit);
  } catch (error) {
    return {
      ok: false,
      reason: "fetch_failed",
      detail: errorToMessage(error)
    };
  }

  const existed = ctx.vault.getAbstractFileByPath(ctx.path) !== null;
  const adapter = new ObsidianVaultAdapter(ctx.vault);
  try {
    if (historical.kind === "text") {
      await adapter.writeText(ctx.path, historical.text);
      return {
        ok: true,
        kind: existed ? "modified" : "created",
        bytes: textByteLength(historical.text)
      };
    }
    await adapter.writeBinary(ctx.path, historical.bytes);
    return {
      ok: true,
      kind: existed ? "modified" : "created",
      bytes: historical.bytes.byteLength
    };
  } catch (error) {
    return {
      ok: false,
      reason: "write_failed",
      detail: errorToMessage(error)
    };
  }
}
