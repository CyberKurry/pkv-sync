import { Notice, TFile, TFolder, type App, type FileManager, type Vault } from "obsidian";
import { isConflictPath } from "./conflict-files";
import { sha256Bytes, sha256TextWithLength } from "./hash";
import type { LocalFileSnapshot, LocalIndex } from "./types";
import { format, type Strings } from "../i18n";
import { debugLog, isTextPath } from "../util";

const SCAN_SNAPSHOT_BATCH_SIZE = 8;

export interface VaultAdapter {
  /** The vault's config folder (`Vault#configDir`), for config-folder paths. */
  readonly configDir: string;
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

/** The Obsidian app surface the adapter needs; accepts `App` itself. */
export type VaultHost = Pick<App, "vault" | "fileManager">;

export interface VaultScanOptions {
  retainPayload?: boolean;
}

export class ObsidianVaultAdapter implements VaultAdapter {
  private readonly vault: Vault;
  private readonly fileManager: FileManager;
  private pruneNotified = false;

  /**
   * Both flags are LIVE callbacks, not constructor-captured values: the
   * settings toggle saves with rebuild:false, so the running engine's
   * adapter must re-read the switch on every trash() (a captured boolean
   * stayed stale until the next engine rebuild), and the Notice must
   * follow the plugin's configured language like every other notice
   * (plugin.text()), not the navigator locale.
   */
  constructor(
    host: VaultHost,
    private readonly pruneEnabled: () => boolean,
    private readonly labels: () => Strings
  ) {
    this.vault = host.vault;
    this.fileManager = host.fileManager;
  }

  get configDir(): string {
    return this.vault.configDir;
  }

  listFiles(): TFile[] {
    const files = this.vault.getFiles();
    // Obsidian's vault.getFiles() reads the file cache, which does not index
    // the config folder. Walk that folder subtree directly so allowlisted
    // config paths (themes, snippets, plugin configs) are discoverable for
    // sync. Paths already returned by getFiles() are deduplicated.
    const known = new Set(files.map((file) => file.path));
    const configFolder = this.vault.getAbstractFileByPath(this.vault.configDir);
    const extra: TFile[] = [];
    if (configFolder instanceof TFolder) {
      collectFilesRecursive(configFolder, known, extra);
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
    // FileManager honours the user's "deleted files" preference (system or
    // local trash); Vault#trash is deprecated in favour of it. D1: folders
    // reuse the SAME entry point as files (spec §4.3) — under a
    // "delete permanently" preference this is no more destructive than the
    // file delete that just happened through the same call.
    if (!file) return;
    await this.fileManager.trashFile(file);
    if (this.pruneEnabled()) await this.pruneEmptyParentsOf(safePath);
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

  /**
   * Best effort: a delete may have emptied the parent chain. Emptiness is
   * read from the filesystem (adapter.list), never TFolder.children — the
   * vault cache does not list dot-entries, so a folder holding only
   * `.nomedia` looks empty there. Never throws: the file delete above
   * already happened and the caller must be able to record it (spec §4.4).
   *
   * Walk consistency: trashFile awaits the physical rename and Obsidian
   * reconciles its cache per adapter operation, so listing the PARENT on
   * the next iteration already reflects the child folder's removal.
   *
   * M-10 exemption (deliberate): the engine discipline checks
   * throwIfAborted() before every vault write/delete, but this walk runs
   * unsupervised after the last check. Acceptable because it stops at the
   * first non-empty parent, touches only folders that are already empty,
   * follows the user's deletion preference (trash by default, so
   * recoverable), and folders never enter the sync index.
   */
  private async pruneEmptyParentsOf(filePath: string): Promise<void> {
    try {
      let dir = parentOf(filePath);
      while (dir !== null) {
        if (startsWithDotRoot(dir) || isUnderConfigDir(dir, this.vault.configDir)) return;
        const folder = this.vault.getAbstractFileByPath(dir);
        if (!(folder instanceof TFolder)) return;
        const listed = await this.vault.adapter.list(dir);
        if (listed.files.length > 0 || listed.folders.length > 0) return;
        await this.fileManager.trashFile(folder);
        this.notifyFirstPrune(dir);
        dir = parentOf(dir);
      }
    } catch (error) {
      debugLog("[pkv-sync] empty-folder prune skipped:", error);
    }
  }

  private notifyFirstPrune(dir: string): void {
    // D2: the prune reorganizes the user's folder tree without them asking;
    // one Notice per adapter lifetime points at the setting to disable it.
    if (this.pruneNotified) return;
    this.pruneNotified = true;
    new Notice(format(this.labels().firstPruneNotice, { path: dir }));
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
  if (hasEscapedPathSeparator(normalized)) return null;
  if (hasUnsafeDecodedShape(normalized)) return null;
  if (hasProtectedRoot(normalized)) return null;
  if (hasWindowsUnsafeComponent(normalized)) return null;
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

/**
 * R11-2 (twin of the server's R10-1 rule): escapes may encode CHARACTERS,
 * never STRUCTURE. A local file literally named `a%2Fb.md` is one segment
 * on disk, but its wire form decodes to the two-level `a/b.md` — a
 * different tree than the vault contains, which the server rejects with
 * WindowsUnsafe. The plugin must agree at scan time so the file is never
 * pushed into a deterministic opaque failure. Re-encoded forms (`%252F`)
 * resurface as `%2F` after one decode round, so every round re-checks —
 * mirroring `introduces_escaped_separator` in server/src/storage/path.rs.
 * Unicode escapes (`%E6%96%87`) encode characters only and stay
 * acceptable.
 */
function hasEscapedPathSeparator(path: string): boolean {
  return /%2f/i.test(path) || /%5c/i.test(path);
}

function hasUnsafeDecodedShape(path: string): boolean {
  let current = path;
  for (let i = 0; i < 4; i++) {
    const decoded = decodeRound(current);
    // R12-2: a well-formed escape whose bytes are not UTF-8 — the server
    // answers InvalidUtf8Escape, so the file can never converge.
    if (decoded === INVALID_UTF8) return true;
    if (decoded === current) {
      // R14-M1: fixed point reached. The server stores every pushed path
      // at its fixed-point decode (decode_to_fixed_point, the idempotent
      // contract), so a literal whose fixed point differs from itself
      // (`a%41.md` → `aA.md`) maps onto a DIFFERENT server path than the
      // local key: a sibling with the decoded spelling collapses onto the
      // same server file with independent index keys — silent mutual
      // overwrite, no conflict sidecar — and the pull re-delivers the
      // file under the decoded name. Refuse to sync it at all.
      return current !== path;
    }
    current = normalizeSeparators(decoded);
    if (unsafeDecodedState(current)) return true;
  }
  // R15-1: the round budget is exhausted and the value is STILL moving.
  // The server's twin (decode_to_fixed_point in server/src/storage/path.rs)
  // runs one final round OUTSIDE its loop: stable → accepted and stored at
  // the fixed point, still changing → PercentEncodingTooDeep → 400
  // invalid_path. Returning `false` here meant "acceptable", which was
  // wrong in both directions: a chain needing exactly 4 rounds was folded
  // into a sibling-colliding server name (R14-M1's failure, one round
  // deeper), and a chain needing 5+ was pushed into a deterministic 400
  // that wedges the whole vault (R15-2). Same final round as the server,
  // same verdict.
  const final = decodeRound(current);
  if (final === INVALID_UTF8) return true;
  if (final === current) {
    // Stable one round past the budget: the server accepts it and stores
    // it at this fixed point, which is exactly R14-M1's folding shape —
    // so the verdict is the same as reaching the fixed point inside the
    // loop, not "accepted because the chain ended".
    return current !== path;
  }
  return true;
}

/** Sentinel for "this round produced bytes that are not valid UTF-8". */
const INVALID_UTF8 = Symbol("invalid_utf8");

const HEX_DIGIT = /^[0-9a-fA-F]$/;
const utf8Encoder = new TextEncoder();
const strictUtf8Decoder = new TextDecoder("utf-8", { fatal: true });

/**
 * One decode round, modelled on the server's `decode_round`.
 *
 * R16-H2: `decodeURIComponent` is ALL-OR-NOTHING — a single malformed
 * triplet makes the whole call throw — while the server's
 * `percent_decode_str` decodes every well-formed `%XX` to a byte, leaves
 * the malformed ones verbatim, and only then requires the whole byte
 * buffer to be valid UTF-8. The difference is not cosmetic: for
 * `a%25zz.md` the server folds to `a%zz.md` (a DIFFERENT stored path than
 * the local key) while `decodeURIComponent` throws and the plugin read
 * that as "nothing left to decode", accepting the file. Two local files
 * then collide on one server path with no conflict sidecar — R14-M1
 * again, reached through a malformed tail instead of the round budget.
 *
 * Returns:
 * - the decoded string when the round succeeded (identical to `value`
 *   when there was nothing to decode),
 * - [`INVALID_UTF8`] when the bytes are not valid UTF-8 (`a%FFb.md`) —
 *   the server answers `InvalidUtf8Escape`, so the caller must refuse.
 */
function decodeRound(value: string): string | typeof INVALID_UTF8 {
  const bytes: number[] = [];
  for (let i = 0; i < value.length; ) {
    if (
      value[i] === "%" &&
      i + 2 < value.length &&
      HEX_DIGIT.test(value[i + 1]) &&
      HEX_DIGIT.test(value[i + 2])
    ) {
      bytes.push(Number.parseInt(value.slice(i + 1, i + 3), 16));
      i += 3;
      continue;
    }
    // Copy the whole code point so astral characters survive as one unit.
    const codePoint = value.codePointAt(i);
    if (codePoint === undefined) break;
    const chunk = String.fromCodePoint(codePoint);
    for (const byte of utf8Encoder.encode(chunk)) bytes.push(byte);
    i += chunk.length;
  }
  try {
    return strictUtf8Decoder.decode(new Uint8Array(bytes));
  } catch {
    return INVALID_UTF8;
  }
}

function unsafeDecodedState(value: string): boolean {
  if (hasEscapedPathSeparator(value)) return true;
  return (
    !isSafePathShape(value) ||
    hasProtectedRoot(value) ||
    startsWithDotRoot(value)
  );
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

function parentOf(path: string): string | null {
  const slash = path.lastIndexOf("/");
  return slash < 0 ? null : path.slice(0, slash);
}

function isUnderConfigDir(dir: string, configDir: string): boolean {
  return dir === configDir || dir.startsWith(`${configDir}/`);
}

/**
 * R14-M3: mirror of the server's `is_windows_unsafe_component`
 * (server/src/storage/path.rs). The server refuses DOS device names
 * (with or without an extension), components containing a colon and
 * trailing dots/spaces on EVERY platform — a Windows client joining
 * later could not materialize them. Without this prefilter the plugin
 * pushes such files into a deterministic 400 invalid_path on every
 * pass, wedging the whole vault's push stream.
 */
const WINDOWS_RESERVED_NAMES = new Set([
  "CON", "PRN", "AUX", "NUL",
  "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
  "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
]);

function hasWindowsUnsafeComponent(path: string): boolean {
  return path.split("/").some((part) => {
    if (part.includes(":")) return true;
    if (part.endsWith(".") || part.endsWith(" ")) return true;
    const stem = part.split(".", 1)[0];
    return WINDOWS_RESERVED_NAMES.has(stem.toUpperCase());
  });
}
