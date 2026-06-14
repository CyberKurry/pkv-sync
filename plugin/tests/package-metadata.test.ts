import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

type JsonObject = Record<string, unknown>;

function readJson(path: string): JsonObject {
  return JSON.parse(readFileSync(resolve(__dirname, "..", path), "utf8")) as JsonObject;
}

function objectField(value: unknown, name: string): JsonObject {
  if (!value || typeof value !== "object") {
    throw new Error(`${name} is not an object`);
  }
  return value as JsonObject;
}

describe("plugin package metadata", () => {
  it("keeps package-lock root versions aligned with package.json", () => {
    const pkg = readJson("package.json");
    const lock = readJson("package-lock.json");
    const packages = objectField(lock.packages, "package-lock packages");
    const root = objectField(packages[""], "package-lock root package");

    expect(lock.version).toBe(pkg.version);
    expect(root.version).toBe(pkg.version);
  });

  it("pins the Obsidian API package to the repo-required version", () => {
    const pkg = readJson("package.json");
    const lock = readJson("package-lock.json");
    const packageDevDeps = objectField(pkg.devDependencies, "package devDependencies");
    const packages = objectField(lock.packages, "package-lock packages");
    const root = objectField(packages[""], "package-lock root package");
    const rootDevDeps = objectField(root.devDependencies, "package-lock root devDependencies");
    const obsidianPackage = objectField(
      packages["node_modules/obsidian"],
      "package-lock obsidian package"
    );

    expect(packageDevDeps.obsidian).toBe("1.12.3");
    expect(rootDevDeps.obsidian).toBe("1.12.3");
    expect(obsidianPackage.version).toBe("1.12.3");
  });
});
