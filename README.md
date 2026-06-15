# PKV Sync

**Self-host your Obsidian vault.** PKV Sync runs on your own server and keeps
your Obsidian vaults in sync across phone, tablet, and desktop. One binary,
one SQLite database, one bare git repo per vault — no cluster, no S3, no
managed cloud. You install it, point Obsidian at it, and your notes sync.

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

Document version: v1.4.2.

English | [简体中文](./README.zh-CN.md) | [繁體中文](./README.zh-Hant.md) | [日本語](./README.ja.md) | [한국어](./README.ko.md)

## Features

- **Multi-user, multi-vault** sync over authenticated devices, with
  per-vault push locks and idempotent retries.
- **Real-time push.** Small edits land sub-second over Server-Sent
  Events; polling stays as a safety net.
- **Git is the source of truth.** Every vault is a bare git repo, so
  per-file history, unified diff, and single-file restore work out of
  the box — in the plugin and in the admin panel.
- **Conflict-safe.** The plugin never silently overwrites local edits;
  conflicts surface as `.conflict-*` files with a one-click resolver.
- **Admin panel** in five languages (English, 简中, 繁中, 日本語, 한국어)
  for users, device tokens, vaults, invites, activity, and blob GC, with
  confirmation dialogs for destructive vault and user actions.
- **AI-readable vaults.** MCP exposes read/write tools over stdio,
  standalone Streamable HTTP, or an embedded `/mcp` route on `pkvsyncd serve`.
- **Bounded by default.** Admin-created passwords use the setup-grade strong
  policy, token secrets are one-time only, uploads and MCP responses are size
  capped, and live SSE streams revalidate revoked tokens.
- **Boring on purpose.** Single binary, single SQLite metadata DB, one
  bare git repo per vault, one content-addressed blob per attachment.

## Quick start with Docker Compose

The recommended path. Caddy in `deploy/caddy/` fronts HTTPS via Let's
Encrypt; PKV Sync sits on `127.0.0.1:6710` inside the compose network and
never sees plain HTTP from the public internet.

You need a domain name (e.g. `sync.example.com`) with A/AAAA records
pointing at the server, and ports `80` and `443` reachable from the
internet (port 80 is needed for ACME HTTP-01 validation).

1. Generate a deployment key:

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. Drop `config.toml` next to `docker-compose.yml`:

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # replace with genkey output
   public_host    = "sync.example.com"   # required, makes admin POSTs work

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker bridge network

   [mcp]
   embed_in_serve = false                # true mounts /mcp on this server
   ```

3. Edit `deploy/caddy/Caddyfile` and replace `sync.example.com` with your
   real domain.

4. Bring the stack up:

   ```bash
   docker compose up -d
   ```

   Open `https://sync.example.com/setup` and create the first
   administrator account in your browser.

5. Install `pkv-sync-plugin.zip` in Obsidian
   (`<vault>/.obsidian/plugins/pkv-sync/`), enable it, paste the share
   URL from the admin panel, then log in or register and pick a vault.

Updating is `docker compose pull && docker compose up -d`. For native
installs, reverse-proxy tuning (Caddy / Nginx / Traefik), `public_host`
semantics, backup / restore, and disk encryption, read the
[deployment hardening guide](./public-docs/deployment-hardening.md).

## MCP deployment modes

PKV Sync exposes the MCP Streamable HTTP transport in two ways. Embedded mode
is opt-in: set `[mcp].embed_in_serve = true` and `pkvsyncd serve` mounts
`/mcp` on the main server port, sharing the same TLS termination, reverse
proxy, deployment key, and bearer token enforcement. Standalone mode keeps the
existing separate process: `pkvsyncd mcp --transport http --bind
127.0.0.1:6711`, useful for air-gapped MCP, dedicated bind addresses, or
independent scaling.

## Obsidian plugin

Local files are the source of truth — the plugin reads and writes your
normal Obsidian vault on disk, no proxy filesystem. Non-sensitive plugin
settings and sync indexes live in
`<vault>/.obsidian/plugins/pkv-sync/data.json`; login state, the active bearer
device token, deployment key, and stable device identity live in Obsidian's
device-local storage instead. Treat Obsidian device-local storage, plaintext
backups, and legacy plugin `data.json` copies as sensitive. Device tokens renew
on use, expire after 90 idle days, and have a 365-day absolute lifetime; logging
in again on the same device rotates the active token.

Day-to-day features — command palette, file history, side-by-side diff,
conflict resolution, selective `.obsidian` sync, device management, and
self-update — are walked through in the
[user manual](./public-docs/user-manual.md).

## Encryption today

PKV Sync 1.0 does **not** yet ship native end-to-end encryption — the
server can read vault contents. Native per-vault E2EE is planned for the
1.x roadmap as an opt-in mode, because encryption trades away the
server-side features (history diff, three-way auto-merge, inline SSE
payload, MCP read/write) that make Git-native PKV useful.

If you need E2EE before it lands, layer
[`git-crypt`](https://github.com/AGWA/git-crypt) on your vault: marked
paths reach the server as ciphertext blobs it cannot decrypt. Filenames
stay plaintext on the server (acceptable for most threat models). Standard
`git clone` and `pkvsyncd materialize` still work for clients that hold
the key.

For real deployments, also run behind HTTPS, restrict
`trusted_proxies`, encrypt the data disk, and encrypt backups — see the
[deployment hardening guide](./public-docs/deployment-hardening.md).

## Looking for…

| Topic | Doc |
| --- | --- |
| Day-to-day plugin usage | [user manual](./public-docs/user-manual.md) |
| Server admin and runtime settings | [admin manual](./public-docs/admin-manual.md) |
| Every CLI command and flag | [CLI reference](./public-docs/cli-reference.md) |
| Upgrading from 0.x to 1.0 | [1.0 upgrade notes](./public-docs/upgrade-notes-v1.0.md) |
| Reverse proxy, TLS, backups, hardening | [deployment hardening](./public-docs/deployment-hardening.md) |
| HTTP API contract | [OpenAPI spec](./public-docs/openapi.yaml) |
| MCP setup and tool list | [MCP how-to](./public-docs/mcp-howto.md) |
| LLM-maintained wiki workflow | [LLM Wiki how-to](./public-docs/llm-wiki-howto.md) |
| Migrating from Obsidian Sync | [migration guide](./public-docs/migrate-from-obsidian-sync.md) |
| Security disclosures | [SECURITY.md](./SECURITY.md) |
| Release history | [CHANGELOG.md](./CHANGELOG.md) |

## Status

PKV Sync 1.4.2 is a security patch from the audit: plugin credentials are removed from synced plugin data and sealed with Electron safeStorage when available, legacy secret-bearing sync indexes are discarded, Git smart HTTP exposes only the main ref and prunes unreachable objects, and server vault path/blob hash validation now fails at shared boundaries.

PKV Sync 1.0 is the first stable release. The public REST API, CLI surface,
storage layout, plugin package, and Docker image are versioned together
under semver: 1.X.Y stays backwards-compatible on the public surface, and
the OpenAPI spec is the canonical compatibility contract. SQLite databases
created by 0.x releases cannot be upgraded in place to 1.0.0 — follow the
[1.0 upgrade notes](./public-docs/upgrade-notes-v1.0.md).

Each GitHub release publishes Linux amd64/arm64 binaries, a Windows x64
binary, a multi-arch GHCR Docker image, the Obsidian plugin zip, and
`SHA256SUMS`.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm --prefix plugin run typecheck
npm --prefix plugin exec vitest run
npm --prefix plugin run build
```

CI runs the full Rust matrix on Linux and Windows, plus plugin
tests/typecheck/build/package, a Docker build, and release-binary smoke
tests.

## License

AGPL-3.0-only. See [LICENSE](./LICENSE).
