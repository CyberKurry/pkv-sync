import { describe, expect, it, vi } from "vitest";
import { SyncEngine, type SyncEngineOptions } from "../../src/sync/engine";
import type { LocalIndex, PullFile } from "../../src/sync/types";

/**
 * R12-4b: `conflictPath` stamps seconds. Two same-path remote conflicts
 * from DIFFERENT commits inside one second used to map to the SAME sidecar
 * path — the second write overwrote the first sidecar's content. The fix
 * appends a nonce when this pass already wrote that exact sidecar path.
 */
function textFile(path: string): PullFile {
  return {
    path,
    file_type: "text",
    content_inline: `content-for-${path}`,
    blob_hash: null,
  } as unknown as PullFile;
}

function makeEngine() {
  const written: Record<string, string> = {};
  const vault = {
    exists: (p: string) => written[p] !== undefined,
    writeText: vi.fn(async (p: string, content: string) => {
      written[p] = content;
    }),
  };
  const engine = new SyncEngine({
    vaultId: "v",
    deviceName: "d",
    vault,
    api: {},
    index: {
      updateIndex: vi.fn(
        async (fn: (i: LocalIndex) => Promise<LocalIndex>) =>
          fn({ lastSyncedCommit: null, files: {} }),
      ),
    },
  } as unknown as SyncEngineOptions);
  const writeRemoteConflict = (
    engine as unknown as {
      writeRemoteConflict: (f: PullFile, c: string) => Promise<void>;
    }
  ).writeRemoteConflict.bind(engine);
  return { writeRemoteConflict, written };
}

describe("R12-4b same-second conflict collision", () => {
  it("keeps both sidecars when two commits conflict in the same second", async () => {
    const { writeRemoteConflict, written } = makeEngine();
    const a = textFile("note.md");
    a.content_inline = "content-from-commit-a";
    const b = textFile("note.md");
    b.content_inline = "content-from-commit-b";
    await writeRemoteConflict(a, "commit-a");
    await writeRemoteConflict(b, "commit-b");
    const paths = Object.keys(written).filter((p) => p.includes("conflict-"));
    expect(paths.length).toBe(2);
    const contents = paths.map((p) => written[p]);
    expect(new Set(contents).size).toBe(2);
  });
});
