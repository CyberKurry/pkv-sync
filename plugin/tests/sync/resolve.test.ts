import { describe, expect, it, vi } from "vitest";
import { acceptLocal, acceptRemote } from "../../src/sync/resolve";
import type { ConflictPair } from "../../src/sync/conflict-files";

function mockVault(overrides: {
  originalExists?: boolean;
  originalContent?: string;
  conflictContent?: string;
} = {}) {
  const deleted: string[] = [];
  const modified: Array<{ path: string; content: string }> = [];
  const created: Array<{ path: string; content: string }> = [];

  const originalFile = { path: "note.md" };
  const conflictFile = {
    path: "note.md.conflict-2026-05-16-143000-abc.md"
  };

  return {
    vault: {
      read: vi
        .fn()
        .mockResolvedValue(overrides.conflictContent ?? "remote content"),
      delete: vi.fn().mockImplementation((f: any) => {
        deleted.push(f.path);
      }),
      modify: vi.fn().mockImplementation((f: any, content: string) => {
        modified.push({ path: f.path, content });
      }),
      create: vi.fn().mockImplementation((path: string, content: string) => {
        created.push({ path, content });
      }),
      getAbstractFileByPath: vi
        .fn()
        .mockImplementation((path: string) =>
          overrides.originalExists !== false ? originalFile : null
        )
    },
    deleted,
    modified,
    created,
    conflictFile
  };
}

describe("acceptLocal", () => {
  it("only deletes the conflict file, original stays", async () => {
    const pair: ConflictPair = {
      originalPath: "note.md",
      conflictPath: "note.md.conflict-2026-05-16-143000-abc.md",
      conflictFile: {
        path: "note.md.conflict-2026-05-16-143000-abc.md"
      } as any
    };
    const { vault, deleted } = mockVault();
    await acceptLocal(vault as any, pair);
    expect(deleted).toContain(
      "note.md.conflict-2026-05-16-143000-abc.md"
    );
    expect(deleted).toHaveLength(1);
  });
});

describe("acceptRemote", () => {
  it("overwrites original with conflict content and deletes conflict file", async () => {
    const pair: ConflictPair = {
      originalPath: "note.md",
      conflictPath: "note.md.conflict-2026-05-16-143000-abc.md",
      conflictFile: {
        path: "note.md.conflict-2026-05-16-143000-abc.md"
      } as any
    };
    const { vault, deleted, modified } = mockVault({
      originalExists: true,
      conflictContent: "remote content"
    });
    await acceptRemote(vault as any, pair);
    expect(modified).toHaveLength(1);
    expect(deleted).toContain(
      "note.md.conflict-2026-05-16-143000-abc.md"
    );
  });

  it("creates original file when it does not exist", async () => {
    const pair: ConflictPair = {
      originalPath: "note.md",
      conflictPath: "note.md.conflict-2026-05-16-143000-abc.md",
      conflictFile: {
        path: "note.md.conflict-2026-05-16-143000-abc.md"
      } as any
    };
    const { vault, deleted, created } = mockVault({
      originalExists: false,
      conflictContent: "remote content"
    });
    await acceptRemote(vault as any, pair);
    expect(created).toHaveLength(1);
    expect(created[0].path).toBe("note.md");
    expect(deleted).toContain(
      "note.md.conflict-2026-05-16-143000-abc.md"
    );
  });
});
