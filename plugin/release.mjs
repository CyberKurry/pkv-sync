import { copyFileSync, mkdirSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join } from "node:path";

const outDir = "release";
const zip = "pkv-sync-plugin.zip";

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

for (const file of ["main.js", "manifest.json", "styles.css"]) {
  copyFileSync(file, join(outDir, file));
}

rmSync(zip, { force: true });

if (process.platform === "win32") {
  execFileSync(
    "powershell",
    [
      "-NoProfile",
      "-Command",
      `Compress-Archive -Path ${outDir}/* -DestinationPath ${zip} -Force`
    ],
    { stdio: "inherit" }
  );
} else {
  execFileSync("zip", ["-r", `../${zip}`, "."], {
    cwd: outDir,
    stdio: "inherit"
  });
}

console.log(zip);
