# PKV Sync

Self-hosted, Git-backed synchronization for Obsidian.

> This repository is the **release mirror** for the PKV Sync Obsidian plugin: the
> plugin source tree and release artifacts, published automatically from the
> [main repository](https://github.com/CyberKurry/pkv-sync) on every release.
> Pull requests are not accepted here — development happens in the main
> repository. Issues are welcome here or in the main repository.

[简体中文](README.zh-CN.md)

## What it is

PKV Sync keeps your vaults in sync across devices through a server **you** host.
Git is the source of truth: every change is a commit, with per-file history,
diff, restore, and vault-level rollback.

## Highlights

- Multi-user, multi-vault sync with sub-second propagation (SSE)
- Full version history: per-file diff/restore and vault-level rollback
- Three-way automatic merge for concurrent text edits; remaining conflicts
  surface as resolvable `.conflict-*` files with an in-app diff view
- Git-native access: read-only `git clone` of your vault over HTTPS with any
  Git tooling
- Built-in MCP server, so your own AI agent can read and maintain your notes
- Admin web UI, Prometheus metrics, and a backup/restore/verify CLI
- Available in English, 简体中文, 繁體中文, 日本語, and 한국어
- One-command migration from Obsidian Sync

## Requirements

This plugin requires a **self-hosted PKV Sync server**. You deploy and operate it
yourself — there is no third-party service, and your notes never leave your own
infrastructure.

## Quick start

1. Deploy the server (single binary or Docker Compose — see the
   [admin manual](https://github.com/CyberKurry/pkv-sync/blob/main/public-docs/admin-manual.md)),
   then open `https://your-host/setup` to create the first admin account.
2. Install **PKV Sync** from the Obsidian community plugin directory and enable it.
3. In the plugin settings, enter your server URL and deployment key, sign in,
   and pick or create a vault. Sync runs automatically; a manual sync command
   is also available.

## Privacy

The plugin communicates only with the server URL you configure. There is no
telemetry and no analytics. The server administrator (you) can read vault
content stored on the server; see the
[security policy](https://github.com/CyberKurry/pkv-sync/blob/main/SECURITY.md)
and the
[deployment hardening guide](https://github.com/CyberKurry/pkv-sync/blob/main/public-docs/deployment-hardening.md)
for details.

## Documentation and support

- [User manual](https://github.com/CyberKurry/pkv-sync/blob/main/public-docs/user-manual.md)
  (also in 简体中文, 繁體中文, 日本語, 한국어)
- [Admin manual](https://github.com/CyberKurry/pkv-sync/blob/main/public-docs/admin-manual.md)
- [Security policy](https://github.com/CyberKurry/pkv-sync/blob/main/SECURITY.md)
- Bug reports and feature requests: open an issue
  [here](https://github.com/CyberKurry/pkv-sync-plugin/issues) or in the
  [main repository](https://github.com/CyberKurry/pkv-sync/issues)

## License

[AGPL-3.0](LICENSE)
