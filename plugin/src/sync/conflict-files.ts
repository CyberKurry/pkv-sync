import type { TFile } from "obsidian";

export interface ConflictFileVault {
  getFiles(): TFile[];
}

/** The app surface these helpers need; accepts `App` itself. */
export interface ConflictFileHost {
  vault: ConflictFileVault;
  fileManager: { trashFile(file: TFile): Promise<void> };
}

interface ConflictFileReader {
  read(file: TFile): Promise<string>;
}

type ConflictPairKind = "remote_copy" | "merge_markers";

/**
 * The ONE grammar every conflict-sidecar name follows (single producer,
 * R13-1/R13-2 audit):
 *
 *   <base>.conflict-YYYY-MM-DD-HHMMSS-<device>[.<ext>]
 *   <base>.conflict-YYYY-MM-DD-HHMMSS-<nonce8>-<device>[.<ext>]
 *
 * The nonce segment is ALWAYS emitted by `conflictPath` since the R13-2
 * fix (a plugin reload used to lose the in-memory collision map, so two
 * same-second sidecars overwrote each other across reloads); the segment
 * stays OPTIONAL when parsing because sidecars written before that fix —
 * and sidecars written by other PKV Sync versions — lack it. Device names
 * are `safeDeviceName` output ([A-Za-z0-9_-]) or the literal "remote";
 * the nonce is 8 chars of [A-Za-z0-9_-]. Neither contains a dot, so the
 * extension is unambiguously "everything after the last dot".
 */
const SIDECAR_RE =
  /^(.+)\.conflict-(\d{4})-(\d{2})-(\d{2})-(\d{6})-(?:([A-Za-z0-9]{8})-)?([A-Za-z0-9_-]+)(\.[^/.]+)?$/;

/** Parsed sidecar name; `nonce` is null for pre-nonce legacy sidecars. */
interface ParsedSidecar {
  originalPath: string;
  nonce: string | null;
  device: string;
}

export function parseConflictPath(path: string): ParsedSidecar | null {
  const name = path.split("/").pop() ?? path;
  const slash = path.lastIndexOf("/");
  const dir = slash >= 0 ? path.slice(0, slash + 1) : "";
  const m = name.match(SIDECAR_RE);
  if (!m) return null;
  return {
    originalPath: `${dir}${m[1]}${m[8] ?? ""}`,
    nonce: m[6] ?? null,
    device: m[7]
  };
}

export function isConflictPath(path: string): boolean {
  return parseConflictPath(path) !== null;
}

export function listConflictFiles(vault: ConflictFileVault): TFile[] {
  return vault.getFiles().filter((file) => isConflictPath(file.path));
}

export async function deleteConflictFiles(
  host: ConflictFileHost
): Promise<number> {
  const files = listConflictFiles(host.vault);
  for (const file of files) {
    await host.fileManager.trashFile(file);
  }
  return files.length;
}

export function originalPathFor(conflictPath: string): string | null {
  return parseConflictPath(conflictPath)?.originalPath ?? null;
}

/**
 * Which version of the file lives in the conflict sidecar (V1).
 *
 * Producers, and what each writes into the sidecar:
 * - engine.writeConflict (local dirty vs incoming remote): the LOCAL
 *   content; the disk keeps the remote. Sidecar role "local".
 * - engine.writeRemoteConflict (local deletion vs remote modification):
 *   the REMOTE content; the disk has nothing. Sidecar role "remote".
 *   The literal device name "remote" cannot collide with a real device
 *   suffix (safeDeviceName maps an empty/unusable name to "device").
 * - server merge (write_text_conflict_sidecar / conflicted three-way
 *   merge): the pushed LOCAL content; the canonical path keeps the
 *   remote. Sidecar role "local".
 *
 * The role decides what "keep local" and "accept remote" mean. With the
 * wrong role, "keep local" trashed the user's only local copy and
 * "accept remote" republished the local draft vault-wide.
 */
export type SidecarRole = "local" | "remote";

export function sidecarRoleFor(conflictPath: string): SidecarRole {
  return parseConflictPath(conflictPath)?.device === "remote"
    ? "remote"
    : "local";
}

export interface ConflictPair {
  originalPath: string;
  conflictPath: string;
  kind: ConflictPairKind;
  conflictFile: TFile;
  /** Which version the sidecar holds; derived from the producer suffix. */
  sidecarRole: SidecarRole;
}

export function hasMergeMarkers(content: string): boolean {
  return (
    content.includes("<<<<<<< local") &&
    content.includes("=======") &&
    content.includes(">>>>>>> remote")
  );
}

export function pairConflicts(
  vault: Pick<ConflictFileVault, "getFiles">
): ConflictPair[] {
  return listConflictFiles(vault)
    .map<ConflictPair | null>((f) => {
      const orig = originalPathFor(f.path);
      return orig
        ? {
            originalPath: orig,
            conflictPath: f.path,
            kind: "remote_copy" as const,
            conflictFile: f,
            sidecarRole: sidecarRoleFor(f.path)
          }
        : null;
    })
    .filter((x): x is ConflictPair => x !== null);
}

async function withKinds(
  vault: ConflictFileReader & Pick<ConflictFileVault, "getFiles">,
  pairs: ConflictPair[]
): Promise<ConflictPair[]> {
  return Promise.all(
    pairs.map(async (pair) => {
      const content = await vault.read(pair.conflictFile);
      return {
        ...pair,
        kind: hasMergeMarkers(content) ? "merge_markers" : "remote_copy"
      };
    })
  );
}

export async function pairConflictsWithKinds(
  vault: Pick<ConflictFileVault, "getFiles"> & ConflictFileReader
): Promise<ConflictPair[]> {
  return withKinds(vault, pairConflicts(vault));
}

export async function findConflictPairsForPathWithKinds(
  vault: Pick<ConflictFileVault, "getFiles"> & ConflictFileReader,
  path: string
): Promise<ConflictPair[]> {
  // Filter first so only the matched conflict files are read for their kind.
  return withKinds(vault, findConflictPairsForPath(vault, path));
}

export function findConflictPairsForPath(
  vault: Pick<ConflictFileVault, "getFiles">,
  path: string
): ConflictPair[] {
  return pairConflicts(vault).filter(
    (pair) => pair.originalPath === path || pair.conflictPath === path
  );
}
