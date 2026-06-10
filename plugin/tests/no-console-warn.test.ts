import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("production plugin logging", () => {
  it("does not leave console.warn calls in production sources", () => {
    for (const path of ["main.ts", "sync/engine.ts"]) {
      const source = readFileSync(resolve(__dirname, `../src/${path}`), "utf8");
      expect(source).not.toContain("console.warn");
    }
  });
});
