#!/usr/bin/env node
// Audit gate for the plugin's dependency tree.
//
// `npm audit --audit-level=moderate` fails on any moderate-or-higher
// advisory. That is the right default, but an advisory with NO published
// patched version cannot be cleared by a bump, and `npm audit fix` in that
// state proposes unrelated major downgrades that do not remove the
// advisory either. This wrapper keeps the gate strict for every other
// advisory and records the single exception with its removal condition.
//
// The exception below is dev-only. The plugin has zero runtime
// dependencies: `main.js` is an esbuild bundle, and every package here is
// a build/test tool. The affected glob input (test-runner file matching)
// is static in this repository.
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

/**
 * GHSA id → why it is tolerated and when the entry must disappear.
 * Removing an entry is the default; adding one needs the same rigour as a
 * code change: dev-only proof, no patched release, and a removal trigger.
 */
const EXCEPTIONS = new Map([
  [
    "GHSA-fc5q-39xc-vjpf",
    {
      package: "picomatch",
      reason: "ReDoS; dev-only vitest/vite chain; no patched release exists",
      removeWhen: "picomatch publishes a version above 4.0.7",
    },
  ],
]);

const GATED = new Set(["moderate", "high", "critical"]);
const pluginDir = fileURLToPath(new URL("..", import.meta.url));

function audit() {
  const npm = process.platform === "win32" ? "npm.cmd" : "npm";
  try {
    return JSON.parse(
      execFileSync(npm, ["audit", "--json"], {
        cwd: pluginDir,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      })
    );
  } catch (error) {
    // npm audit exits non-zero whenever the report contains vulnerabilities;
    // the JSON on stdout is still the answer.
    if (error.stdout) return JSON.parse(error.stdout);
    throw error;
  }
}

function collect(report) {
  const advisories = new Map();
  for (const bucket of Object.values(report.vulnerabilities ?? {})) {
    if (!GATED.has(bucket.severity)) continue;
    for (const via of bucket.via ?? []) {
      if (typeof via !== "object" || !via.url) continue;
      const id = via.url.split("/").pop();
      if (!advisories.has(id)) {
        advisories.set(id, { title: via.title, severity: bucket.severity });
      }
    }
  }
  return advisories;
}

const advisories = collect(audit());
const failures = [];
for (const [id, advisory] of advisories) {
  if (!EXCEPTIONS.has(id)) {
    failures.push(`${id} (${advisory.severity}): ${advisory.title}`);
  }
}
for (const [id, entry] of EXCEPTIONS) {
  if (!advisories.has(id)) {
    console.warn(
      `audit-gate: exception ${id} is stale (${entry.removeWhen}); remove it from scripts/audit-gate.mjs`
    );
  }
}

if (failures.length > 0) {
  console.error("audit-gate: moderate+ advisories without an exception:");
  for (const failure of failures) {
    console.error(`  ${failure}`);
  }
  process.exit(1);
}

if (advisories.size === 0) {
  console.log("audit-gate: 0 moderate+ advisories");
} else {
  const ids = [...advisories.keys()].sort().join(", ");
  console.log(`audit-gate: passed (allowlisted: ${ids})`);
}
