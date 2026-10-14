import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import * as ts from "typescript";

/**
 * Behavioral dead-export guard (R2-8).
 *
 * The previous version of this test was a source-text assertion —
 * `expect(source("x.ts")).not.toMatch(/export function deadSymbol\b/)` —
 * which can only protect symbols someone thought to hand-list, and never
 * catches a NEW dead export. It let a newly added zero-call-site engine
 * method ship undetected, which is exactly the failure mode a guard
 * exists to prevent.
 *
 * This test instead ENUMERATES what the module actually exports and
 * asserts every exported identifier is consumed somewhere outside its own
 * declaration:
 *
 *   1. Parse every TypeScript file under src/ with the TypeScript compiler
 *      API (`ts.createSourceFile`) and collect the names of all top-level
 *      exported declarations (function / class / interface / type alias /
 *      enum / const-let-var) — plus the public member names of exported
 *      classes (this is how the engine method slipped through: a class
 *      METHOD, invisible to any top-level scan).
 *   2. For each name, look for a non-definition occurrence:
 *        a. in the DECLARING file with EVERY declaration of that name blanked
 *           out (an internal consumer — types used by other exported
 *           signatures, constants used by other functions). Masking all
 *           same-named declarations, not just one, is what keeps a twin from
 *           vouching for a dead member (R3-9), or
 *        b. in any OTHER file under `src/` or `tests/`.
 *      Comments are stripped before matching, so a name mentioned only in
 *      prose does not count as usage (this file is excluded from its own
 *      scan for the same reason — its comments describe the symbols it
 *      polices).
 *
 * There is no allow-list of "expected" names: the existence check is the
 * whole test, so the next zero-call-site addition fails CI with the
 * offender named. (An allow-list of genuinely-consumed exports was
 * considered and rejected: it would need to track ~200 names by hand and
 * rot in exactly the same way the regex list did.)
 *
 * `default` (the Obsidian plugin entry) is exempt: it is consumed by the
 * esbuild bundle, not by any import in this tree. The `onload`/`onunload`
 * lifecycle methods are exempt the same way — Obsidian's Plugin
 * superclass calls them reflectively from outside the tree.
 */

const pluginRoot = resolve(__dirname, "..");
const srcRoot = join(pluginRoot, "src");

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      out.push(...walk(full));
    } else if (full.endsWith(".ts")) {
      out.push(full);
    }
  }
  return out;
}

/** Blank out block comments and line comments, preserving line breaks. */
function stripComments(text: string): string {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, " "))
    .replace(/(^|[^:])\/\/.*$/gm, "$1");
}

interface CollectedExport {
  name: string;
  file: string;
  /** Source with this export's whole top-level statement blanked out. */
  declaringFileMasked: string;
}

function isExported(node: ts.Node): boolean {
  return (
    ts.canHaveModifiers(node) &&
    !!ts.getModifiers(node)?.some(
      (modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword
    )
  );
}

function hasNonPublicModifier(node: ts.Node): boolean {
  return (
    ts.canHaveModifiers(node) &&
    !!ts.getModifiers(node)?.some(
      (modifier) =>
        modifier.kind === ts.SyntaxKind.PrivateKeyword ||
        modifier.kind === ts.SyntaxKind.ProtectedKeyword
    )
  );
}

function collectExports(): CollectedExport[] {
  const collected: CollectedExport[] = [];
  for (const file of walk(srcRoot)) {
    const text = readFileSync(file, "utf8");
    const sourceFile = ts.createSourceFile(file, text, ts.ScriptTarget.ES2020, true);
    // Every declaration in this file that introduces `name`, so masking
    // blanks all of them at once (R3-9). Blanking only the entry under test
    // left a same-named twin behind for the `\bname\b` probe to match: two
    // classes in one file may each declare `render`, and a method may have
    // overload signatures, so a genuinely dead member looked used because its
    // twin satisfied the regex. Masking the whole name is the correct
    // semantics — overload signatures and a dead/used pair both mean "is this
    // NAME reached from anywhere else", not "is this statement".
    const rangesByName = new Map<string, Array<{ start: number; end: number }>>();
    const addDeclaration = (name: string, node: ts.Node) => {
      const ranges = rangesByName.get(name) ?? [];
      ranges.push({ start: node.getStart(sourceFile), end: node.end });
      rangesByName.set(name, ranges);
    };
    for (const stmt of sourceFile.statements) {
      if (!isExported(stmt)) continue;
      const names: string[] = [];
      if ((ts.isFunctionDeclaration(stmt) || ts.isClassDeclaration(stmt) || ts.isInterfaceDeclaration(stmt) || ts.isTypeAliasDeclaration(stmt) || ts.isEnumDeclaration(stmt)) && stmt.name) {
        names.push(stmt.name.text);
      } else if (ts.isVariableStatement(stmt)) {
        for (const decl of stmt.declarationList.declarations) {
          if (ts.isIdentifier(decl.name)) names.push(decl.name.text);
        }
      }
      for (const name of names) {
        addDeclaration(name, stmt);
      }
      // Public members of exported classes: a class METHOD is invisible to
      // any top-level scan, which is how the zero-call-site engine method
      // slipped through.
      if (ts.isClassDeclaration(stmt)) {
        for (const member of stmt.members) {
          if (!member.name || !ts.isIdentifier(member.name)) continue;
          if (member.kind === ts.SyntaxKind.Constructor) continue;
          if (hasNonPublicModifier(member)) continue;
          addDeclaration(member.name.text, member);
        }
      }
    }
    // Blank every range for a name, longest-start-first so the indices of the
    // remaining ranges stay valid (a class range can enclose a member range
    // when the member shares the class's name).
    for (const [name, ranges] of rangesByName) {
      let masked = text;
      for (const range of [...ranges].sort((a, b) => b.start - a.start)) {
        masked =
          masked.slice(0, range.start) +
          " ".repeat(range.end - range.start) +
          masked.slice(range.end);
      }
      collected.push({
        name,
        file: relative(srcRoot, file),
        declaringFileMasked: masked
      });
    }
  }
  return collected;
}

/** Every non-declaring source/test file, comment-stripped (cached on first use). */
function otherFilesTexts(excludeFile: string): string[] {
  const files = [...walk(srcRoot), ...walk(join(pluginRoot, "tests"))].filter(
    (f) => f !== excludeFile && f !== import.meta.filename
  );
  return files.map((f) => stripComments(readFileSync(f, "utf8")));
}

const cache = new Map<string, string[]>();

function usagesElsewhere(exportInfo: CollectedExport): string[] {
  const absolute = join(srcRoot, exportInfo.file);
  let texts = cache.get(absolute);
  if (!texts) {
    texts = otherFilesTexts(absolute);
    cache.set(absolute, texts);
  }
  return texts.filter((t) => new RegExp(`\\b${exportInfo.name}\\b`).test(t));
}

/**
 * Names whose consumer lives OUTSIDE this tree. Everything here must carry
 * a reason — an unexplained entry is exactly the rot this test exists to
 * prevent.
 */
const FRAMEWORK_CONSUMED = new Set([
  // Obsidian's Plugin superclass invokes these lifecycle hooks by name.
  "onload",
  "onunload"
]);

describe("plugin dead export cleanup", () => {
  it("every exported identifier is consumed outside its own declaration", () => {
    const exports = collectExports();
    expect(exports.length).toBeGreaterThan(150); // sanity: the walk found the module

    const dead: string[] = [];
    for (const exportInfo of exports) {
      if (exportInfo.name === "default") continue; // consumed by the esbuild bundle
      if (FRAMEWORK_CONSUMED.has(exportInfo.name)) continue;
      const internalUse = new RegExp(`\\b${exportInfo.name}\\b`).test(
        stripComments(exportInfo.declaringFileMasked)
      );
      const externalUse = usagesElsewhere(exportInfo).length > 0;
      if (!internalUse && !externalUse) {
        dead.push(`${exportInfo.name} (${exportInfo.file})`);
      }
    }

    expect(
      dead,
      "Dead exports — nothing under src/ or tests/ consumes these outside " +
        "their own declaration. Either remove the export, or consume it, or " +
        "(if it is genuinely part of the module's public surface) import it " +
        "from a test so the contract is pinned. " +
        "(Guard added by R2-8; its predecessor was a source-text regex that " +
        "could not catch NEW dead exports and shipped one undetected.)"
    ).toEqual([]);
  });
});
