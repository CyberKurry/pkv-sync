import esbuild from "esbuild";
import process from "process";
import { builtinModules as builtins } from "node:module";
import { readFileSync } from "fs";

// Read version from package.json so the build artefact (main.js) carries the
// release version in its leading banner. Without this, esbuild's output is
// byte-identical across version bumps and git cannot tell whether main.js was
// rebuilt for the new release. Embedding the version makes "did I rebuild?"
// trivially auditable from the bundle itself.
const pkg = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf-8"));

const prod = process.argv[2] === "production";
const watch = process.argv.includes("--watch");

const ctx = await esbuild.context({
  banner: { js: `/* PKV Sync Obsidian Plugin v${pkg.version} */` },
  entryPoints: ["src/main.ts"],
  bundle: true,
  external: ["obsidian", "electron", "@codemirror/*", "@lezer/*", ...builtins],
  format: "cjs",
  target: "es2018",
  logLevel: "info",
  sourcemap: prod ? false : "inline",
  treeShaking: true,
  outfile: "main.js",
  minify: prod
});

if (watch) {
  await ctx.watch();
} else {
  await ctx.rebuild();
  await ctx.dispose();
}
