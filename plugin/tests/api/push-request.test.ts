import { expect, it } from "vitest";
import {
  makePushBody,
  pushBodyBytes,
  pushChangeBytes
} from "../../src/api/push-request";
import {
  LEGACY_PUSH_LIMITS,
  normalizePushLimits
} from "../../src/sync/push-limits";
import type { PushChange } from "../../src/sync/types";

it.each(["hello", "中文😀", "\"\\\n\t\u0000", "\ud800"])(
  "counts the actual serialized UTF-8 body for %j",
  (content) => {
    const change = { kind: "text" as const, path: "目录/quoted-\".md", content };
    const body = makePushBody([change], "设备😀");
    expect(pushBodyBytes([change], "设备😀")).toBe(
      new TextEncoder().encode(JSON.stringify(body)).byteLength
    );
  }
);

it("counts a long path exactly", () => {
  const path = `${"深/".repeat(200)}note.md`;
  const change: PushChange = { kind: "text", path, content: "x" };
  expect(pushBodyBytes([change], "设备😀")).toBe(
    new TextEncoder().encode(
      JSON.stringify({ changes: [change], device_name: "设备😀" })
    ).byteLength
  );
});

it("counts a blob change without mime", () => {
  const change: PushChange = {
    kind: "blob",
    path: "attachments/image.png",
    blob_hash: "hash",
    size: 3
  };
  expect(pushChangeBytes(change)).toBe(
    new TextEncoder().encode(JSON.stringify(change)).byteLength
  );
  expect(pushBodyBytes([change], "laptop")).toBe(
    new TextEncoder().encode(
      JSON.stringify({ changes: [change], device_name: "laptop" })
    ).byteLength
  );
});

it("counts the empty batch body exactly", () => {
  expect(pushBodyBytes([], "laptop")).toBe(
    new TextEncoder().encode(
      JSON.stringify({ changes: [], device_name: "laptop" })
    ).byteLength
  );
});

it.each([undefined, null, {}, "limits", 7, []])(
  "falls back to the legacy limits for %j",
  (raw) => {
    expect(normalizePushLimits(raw)).toEqual({
      max_json_bytes: LEGACY_PUSH_LIMITS.max_json_bytes,
      max_changes: LEGACY_PUSH_LIMITS.max_changes
    });
  }
);

it.each([
  ["missing", {}],
  ["null", { max_json_bytes: null, max_changes: null }],
  ["negative", { max_json_bytes: -1, max_changes: -1 }],
  ["zero", { max_json_bytes: 0, max_changes: 0 }],
  ["NaN", { max_json_bytes: Number.NaN, max_changes: Number.NaN }],
  ["Infinity", { max_json_bytes: Number.POSITIVE_INFINITY, max_changes: Number.NEGATIVE_INFINITY }],
  ["fraction", { max_json_bytes: 1.5, max_changes: 2.5 }],
  ["unsafe integer", { max_json_bytes: Number.MAX_SAFE_INTEGER + 1, max_changes: Number.MAX_SAFE_INTEGER + 2 }]
])("uses the legacy value for each invalid %s field", (_label, raw) => {
  expect(normalizePushLimits(raw)).toEqual({
    max_json_bytes: LEGACY_PUSH_LIMITS.max_json_bytes,
    max_changes: LEGACY_PUSH_LIMITS.max_changes
  });
});

it("keeps valid limits and caps the change count at 1000", () => {
  expect(
    normalizePushLimits({ max_json_bytes: 512_000, max_changes: 20 })
  ).toEqual({ max_json_bytes: 512_000, max_changes: 20 });
  expect(
    normalizePushLimits({ max_json_bytes: 512_000, max_changes: 100_000 })
  ).toEqual({ max_json_bytes: 512_000, max_changes: 1000 });
});
