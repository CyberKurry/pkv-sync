import type { PushChange } from "./types";
import type { LocalFileSnapshot } from "./types";
import { guessMime } from "./mime";

/**
 * Convert a local snapshot into the wire change for a push, and give it a
 * fingerprint for deduplication/cooldown identity.
 *
 * Extracted from two byte-identical copies (engine + migration, R7-r3
 * Dup-1): one producer for the change shape AND its fingerprint, so the
 * two can never drift apart again.
 */
export function snapshotToPushChange(file: LocalFileSnapshot): PushChange {
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

/**
 * Identity of a push item for cooldown/dedup purposes: kind + path + hash
 * + size. A changed file gets a new fingerprint; an edit that returns to
 * the cooled content is recognized as the same item.
 */
export function pushItemFingerprint(file: LocalFileSnapshot): string {
  return JSON.stringify([file.kind, file.path, file.hash, file.size]);
}
