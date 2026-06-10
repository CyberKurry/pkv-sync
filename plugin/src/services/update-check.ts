import { requestUrl, type RequestUrlParam } from "obsidian";
import { ApiClient } from "../api/client";
import { sha256Text } from "../sync/hash";
import { errorToMessage } from "../util";

export type UpdateSource = "server" | "github";

export interface PluginUpdateStatus {
  version: string;
  source: UpdateSource;
  releaseNotesUrl: string;
  mainJsUrl: string;
  mainJsSha256: string;
  manifestJsonUrl: string;
  manifestJsonSha256: string | null;
  stylesCssUrl: string | null;
  stylesCssSha256: string | null;
}

export interface PluginFileAdapter {
  read(path: string): Promise<string>;
  write(path: string, data: string): Promise<void>;
  remove(path: string): Promise<void>;
  mkdir?(path: string): Promise<void>;
}

interface RecoverPendingUpdateOptions {
  adapter: PluginFileAdapter;
  configDir: string;
  pluginId: string;
  pluginDir?: string;
}

interface UpdateCheckOptions {
  api: ApiClient;
  adapter: PluginFileAdapter;
  configDir: string;
  currentVersion: string;
  pluginId?: string;
  pluginDir?: string;
  githubRepo?: string;
}

interface ServerPluginManifest {
  version: string;
  main_js_url: string;
  main_js_sha256: string;
  manifest_json_url: string;
  manifest_json_sha256?: string | null;
  styles_css_url?: string | null;
  styles_css_sha256?: string | null;
}

interface GitHubAsset {
  name: string;
  browser_download_url: string;
}

interface GitHubRelease {
  tag_name: string;
  html_url: string;
  body?: string;
  assets?: GitHubAsset[];
}

const DEFAULT_GITHUB_REPO = "cyberkurry/pkv-sync";
const UPDATE_REQUEST_TIMEOUT_MS = 30_000;

type TimedRequestUrlParam = RequestUrlParam & { requestTimeout: number };

function withUpdateTimeout(request: RequestUrlParam): TimedRequestUrlParam {
  return { ...request, requestTimeout: UPDATE_REQUEST_TIMEOUT_MS };
}

export class UpdateCheckService {
  private readonly pluginId: string;
  private readonly githubRepo: string;

  constructor(private readonly options: UpdateCheckOptions) {
    this.pluginId = options.pluginId ?? "pkv-sync";
    this.githubRepo = options.githubRepo ?? DEFAULT_GITHUB_REPO;
  }

  async checkOnce(source: UpdateSource): Promise<PluginUpdateStatus | null> {
    if (source === "github") return this.fetchGitHubManifest();
    try {
      const serverUpdate = await this.fetchServerManifest();
      if (serverUpdate) return serverUpdate;
      return null;
    } catch {
      return this.fetchGitHubManifest();
    }
  }

  async applyUpdate(update: PluginUpdateStatus): Promise<void> {
    const mainJs = await this.downloadVerified(
      update.mainJsUrl,
      update.mainJsSha256,
      "main.js"
    );
    const manifestJson = await this.downloadVerified(
      update.manifestJsonUrl,
      update.manifestJsonSha256,
      "manifest.json"
    );
    const stylesCss =
      update.stylesCssUrl === null
        ? null
        : await this.downloadVerified(
            update.stylesCssUrl,
            update.stylesCssSha256,
            "styles.css"
          );

    await this.writePluginFile("main.js", mainJs);
    await this.writePluginFile("manifest.json", manifestJson);
    if (stylesCss !== null) {
      await this.writePluginFile("styles.css", stylesCss);
    }
  }

  private async fetchServerManifest(): Promise<PluginUpdateStatus | null> {
    const manifest = await this.options.api.request<ServerPluginManifest>(
      "GET",
      "/api/plugin-manifest",
      undefined,
      true
    );
    const version = normalizeStableVersion(manifest.version);
    if (!version || compareVersions(version, this.options.currentVersion) <= 0) {
      return null;
    }
    return {
      version,
      source: "server",
      releaseNotesUrl: `https://github.com/${DEFAULT_GITHUB_REPO}/releases/tag/v${version}`,
      mainJsUrl: manifest.main_js_url,
      mainJsSha256: manifest.main_js_sha256,
      manifestJsonUrl: manifest.manifest_json_url,
      manifestJsonSha256: manifest.manifest_json_sha256 ?? null,
      stylesCssUrl: manifest.styles_css_url ?? null,
      stylesCssSha256: manifest.styles_css_sha256 ?? null
    };
  }

  private async fetchGitHubManifest(): Promise<PluginUpdateStatus | null> {
    const response = await requestUrl(
      withUpdateTimeout({
        url: `https://api.github.com/repos/${this.githubRepo}/releases/latest`,
        method: "GET",
        headers: {
          "User-Agent": `PKVSync-Plugin/${this.options.currentVersion}`
        },
        throw: false
      })
    );
    if (response.status < 200 || response.status >= 300) return null;

    const release = JSON.parse(response.text) as GitHubRelease;
    const version = normalizeStableVersion(release.tag_name);
    if (!version || compareVersions(version, this.options.currentVersion) <= 0) {
      return null;
    }
    const assets = release.assets ?? [];
    const main = findAsset(assets, "main.js");
    const manifest = findAsset(assets, "manifest.json");
    if (!main || !manifest) return null;
    const styles = findAsset(assets, "styles.css");
    const notes = release.body ?? "";
    const mainSha256 = extractSha256(notes, "main.js");
    const manifestSha256 = extractSha256(notes, "manifest.json");
    const stylesSha256 = styles ? extractSha256(notes, "styles.css") : null;
    if (!mainSha256 || !manifestSha256 || (styles && !stylesSha256)) {
      return null;
    }

    return {
      version,
      source: "github",
      releaseNotesUrl: release.html_url,
      mainJsUrl: main.browser_download_url,
      mainJsSha256: mainSha256,
      manifestJsonUrl: manifest.browser_download_url,
      manifestJsonSha256: manifestSha256,
      stylesCssUrl: styles?.browser_download_url ?? null,
      stylesCssSha256: stylesSha256
    };
  }

  private async downloadVerified(
    url: string,
    expectedSha256: string | null,
    fileName: string
  ): Promise<string> {
    const serverPath = this.serverPath(url);
    const text =
      serverPath === null
        ? await this.downloadPublicText(url, fileName)
        : await this.options.api.requestText(serverPath, true);
    if (text.length === 0) {
      throw new Error(`Downloaded ${fileName} is empty`);
    }
    if (expectedSha256) {
      const actual = await sha256Text(text);
      if (actual !== expectedSha256.toLowerCase()) {
        throw new Error(`Downloaded ${fileName} sha256 mismatch`);
      }
    }
    return text;
  }

  private async downloadPublicText(url: string, fileName: string): Promise<string> {
    const response = await requestUrl(
      withUpdateTimeout({
        url,
        method: "GET",
        throw: false
      })
    );
    if (response.status < 200 || response.status >= 300) {
      throw new Error(`Failed to download ${fileName}: HTTP ${response.status}`);
    }
    return response.text;
  }

  private serverPath(url: string): string | null {
    try {
      const remote = new URL(url);
      const server = new URL(this.options.api.serverUrl());
      if (remote.origin !== server.origin) return null;
      return `${remote.pathname}${remote.search}`;
    } catch {
      return null;
    }
  }

  private async writePluginFile(fileName: string, content: string): Promise<void> {
    const pluginDir = pluginDirectory({
      configDir: this.options.configDir,
      pluginId: this.pluginId,
      pluginDir: this.options.pluginDir
    });
    await ensureDirectory(this.options.adapter, pluginDir);
    const target = resolvePluginAssetPathInDirectory(pluginDir, fileName);
    const temp = resolvePluginAssetPathInDirectory(pluginDir, `.${fileName}.new`);
    const backup = resolvePluginAssetPathInDirectory(pluginDir, `.${fileName}.bak`);
    const expected = resolvePluginAssetPathInDirectory(
      pluginDir,
      `.${fileName}.sha256`
    );
    const expectedSha256 = await sha256Text(content);
    await this.options.adapter.write(temp, content);
    await this.options.adapter.write(expected, expectedSha256);
    let hadBackup = false;
    try {
      const old = await this.options.adapter.read(target);
      await this.options.adapter.write(backup, old);
      hadBackup = true;
    } catch {
      await this.options.adapter.remove(backup).catch(() => undefined);
    }
    await this.options.adapter.write(target, content);
    const actualSha256 = await sha256Text(await this.options.adapter.read(target));
    if (actualSha256 !== expectedSha256) {
      if (hadBackup) {
        const old = await this.options.adapter.read(backup);
        await this.options.adapter.write(target, old);
      } else {
        await this.options.adapter.remove(target).catch(() => undefined);
      }
      await Promise.allSettled([
        this.options.adapter.remove(temp),
        this.options.adapter.remove(expected)
      ]);
      throw new Error(`${fileName} post-write sha256 mismatch`);
    }
    await this.options.adapter.remove(temp).catch(() => undefined);
    await this.options.adapter.remove(expected).catch(() => undefined);
  }
}

export async function recoverPendingUpdate(
  options: RecoverPendingUpdateOptions
): Promise<void> {
  for (const fileName of ["main.js", "manifest.json", "styles.css"]) {
    await recoverPendingFile(options, fileName);
  }
}

export function compareVersions(left: string, right: string): number {
  const a = parseVersion(left);
  const b = parseVersion(right);
  if (!a || !b) return left.localeCompare(right);
  const partsCompare =
    a.parts[0] - b.parts[0] ||
    a.parts[1] - b.parts[1] ||
    a.parts[2] - b.parts[2];
  if (partsCompare !== 0) return partsCompare;
  if (a.prerelease === b.prerelease) return 0;
  return a.prerelease ? -1 : 1;
}

function resolvePluginAssetPathInDirectory(pluginDir: string, fileName: string): string {
  if (
    fileName.includes("/") ||
    fileName.includes("\\") ||
    fileName.includes("..")
  ) {
    throw new Error(`unsafe plugin asset path: ${fileName}`);
  }
  return `${trimSlashes(pluginDir)}/${fileName}`;
}

function normalizeStableVersion(value: string): string | null {
  const version = value.trim().replace(/^v/i, "");
  if (!version || version.includes("-")) return null;
  return parseVersion(version) ? version : null;
}

function parseVersion(
  value: string
): { parts: [number, number, number]; prerelease: boolean } | null {
  const [core, suffix] = value.trim().replace(/^v/i, "").split("-", 2);
  const pieces = core.split(".");
  if (pieces.length > 3 || pieces.length === 0) return null;
  const numbers = pieces.map((part) => Number(part));
  if (numbers.some((part) => !Number.isInteger(part) || part < 0)) return null;
  return {
    parts: [numbers[0] ?? 0, numbers[1] ?? 0, numbers[2] ?? 0],
    prerelease: suffix !== undefined
  };
}

function findAsset(
  assets: GitHubAsset[],
  name: string
): GitHubAsset | undefined {
  return assets.find((asset) => asset.name === name);
}

function extractSha256(notes: string, fileName: string): string | null {
  const escaped = fileName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const sha = "([a-fA-F0-9]{64})";
  const shaBeforeFile = new RegExp(`\\b${sha}\\b[^\\n]*${escaped}`);
  const fileBeforeSha = new RegExp(`${escaped}[^\\n]*\\b${sha}\\b`);
  return (
    shaBeforeFile.exec(notes)?.[1]?.toLowerCase() ??
    fileBeforeSha.exec(notes)?.[1]?.toLowerCase() ??
    null
  );
}

function trimSlashes(value: string): string {
  return value.replace(/[\\/]+$/g, "");
}

function fallbackPluginDirectory(configDir: string, pluginId: string): string {
  return `${trimSlashes(configDir)}/plugins/${pluginId}`;
}

function pluginDirectory(options: {
  configDir: string;
  pluginId: string;
  pluginDir?: string;
}): string {
  const dir = options.pluginDir?.trim();
  return dir ? trimSlashes(dir) : fallbackPluginDirectory(options.configDir, options.pluginId);
}

async function ensureDirectory(
  adapter: PluginFileAdapter,
  path: string
): Promise<void> {
  if (!adapter.mkdir) return;
  try {
    await adapter.mkdir(path);
  } catch (error) {
    if (!isAlreadyExistsError(error)) throw error;
  }
}

function isAlreadyExistsError(error: unknown): boolean {
  const message = errorToMessage(error);
  return /\b(EEXIST|already exists|exists)\b/i.test(message);
}

async function recoverPendingFile(
  options: RecoverPendingUpdateOptions,
  fileName: string
): Promise<void> {
  const dir = pluginDirectory(options);
  const target = resolvePluginAssetPathInDirectory(dir, fileName);
  const temp = resolvePluginAssetPathInDirectory(dir, `.${fileName}.new`);
  const expected = resolvePluginAssetPathInDirectory(dir, `.${fileName}.sha256`);

  let staged: string;
  let expectedSha256: string;
  try {
    staged = await options.adapter.read(temp);
    expectedSha256 = (await options.adapter.read(expected)).trim().toLowerCase();
  } catch {
    return;
  }

  if ((await sha256Text(staged)) === expectedSha256) {
    await options.adapter.write(target, staged);
  }
  await options.adapter.remove(temp).catch(() => undefined);
  await options.adapter.remove(expected).catch(() => undefined);
}
