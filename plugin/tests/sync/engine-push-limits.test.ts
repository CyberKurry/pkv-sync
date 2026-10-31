import { beforeEach, expect, it, vi } from "vitest";
import { requestUrl } from "obsidian";
import { ApiError } from "../../src/api/client";
import type { PushResponse } from "../../src/sync/types";
import { createPushFixture, textSnapshots } from "../helpers/push-fixture";

beforeEach(() => {
  vi.mocked(requestUrl).mockReset();
});

it("syncs 722 changes without sending a JSON request over the soft target", async () => {
  const f = createPushFixture(textSnapshots(721, 3300, true));
  await expect(f.engine.syncNow()).resolves.toBeUndefined();
  const pushes = f.calls.filter((call) => call.path.endsWith("/push"));
  expect(pushes).toHaveLength(3);
  expect(pushes.every((call) => call.status === 200 && call.bytes <= 1_048_576)).toBe(true);
  expect(pushes.reduce((sum, call) => sum + call.count, 0)).toBe(722);
  expect(Object.keys(f.index.value.files)).toHaveLength(722);
});

it("does not resend an accepted prefix when a later sub-batch fails", async () => {
  const f = createPushFixture(textSnapshots(721, 3300, true));
  const original = f.api.push.bind(f.api);
  let attempts = 0;
  vi.spyOn(f.api, "push").mockImplementation(
    async (vaultId, ifMatch, changes, deviceName) => {
      attempts += 1;
      if (attempts === 2) throw new ApiError(500, "boom", "boom");
      return original(vaultId, ifMatch, changes, deviceName);
    }
  );

  await expect(f.engine.syncNow()).rejects.toMatchObject({ status: 500 });
  const accepted = f.calls.filter(
    (call) => call.path.endsWith("/push") && call.status === 200
  );
  expect(accepted).toHaveLength(1);
  const acceptedHash = accepted[0].bodyHash;
  const acceptedCount = accepted[0].count;
  expect(Object.keys(f.index.value.files)).toHaveLength(acceptedCount);

  vi.mocked(f.api.push).mockRestore();
  await expect(f.engine.syncNow()).resolves.toBeUndefined();
  expect(Object.keys(f.index.value.files)).toHaveLength(722);
  const resent = f.calls.filter(
    (call) => call.path.endsWith("/push") && call.bodyHash === acceptedHash
  );
  expect(resent).toHaveLength(1);
});

it("keeps the old head after an all-clean auto-merge so the pull covers the merge range", async () => {
  const f = createPushFixture(textSnapshots(3, 100));
  const merged: PushResponse = {
    new_commit: "merged",
    files_changed: 3,
    merge_outcomes: [
      { path: "note-0000.md", outcome: "clean" },
      { path: "note-0001.md", outcome: "clean" },
      { path: "note-0002.md", outcome: "clean" }
    ]
  };
  vi.spyOn(f.api, "push").mockResolvedValueOnce(merged);

  await expect(f.engine.syncNow()).resolves.toBeUndefined();

  expect(f.index.value.lastSyncedCommit).toBeNull();
  expect(Object.keys(f.index.value.files)).toHaveLength(3);
});

it("does not resend a cooling file when another file becomes pending", async () => {
  const files = textSnapshots(1, 600_000);
  const f = createPushFixture(files);
  f.setBodyLimit(512_000);
  await expect(f.engine.syncNow()).rejects.toMatchObject({
    detail: { kind: "remote_singleton", path: "note-0000.md" }
  });
  const first = f.calls.filter((call) => call.path.endsWith("/push"));
  expect(first).toHaveLength(1);
  files.push({ ...textSnapshots(1, 1)[0], path: "new.md" });

  await expect(f.engine.syncNow()).rejects.toMatchObject({
    detail: { kind: "remote_singleton", path: "note-0000.md" }
  });

  expect(f.calls.filter((call) => call.path.endsWith("/push"))).toEqual(first);
  expect(f.index.value).toEqual({ lastSyncedCommit: null, files: {} });

  f.setBodyLimit(2_097_152);
  await expect(f.engine.syncNow({ retryOversized: true })).resolves.toBeUndefined();
  expect(Object.keys(f.index.value.files)).toHaveLength(2);
});
