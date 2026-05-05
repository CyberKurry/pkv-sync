# PKV Sync

Self-hosted Obsidian vault sync with a Rust server, SQLite metadata, Git-backed
vault history, and an Obsidian plugin.

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

English | [简体中文](./README.zh-CN.md)

## Status

PKV Sync is pre-1.0 software. APIs, storage layout, release packaging, and
operational defaults may still change.

The current design does not provide end-to-end encryption. The server can read
vault contents. Use HTTPS, strict access control, encrypted disks, and encrypted
backups for real deployments.

## What It Includes

- `pkvsyncd`: the server daemon and CLI
- `pkv-sync`: the Obsidian desktop/mobile plugin
- SQLite metadata under the configured data directory
- Per-vault Git repositories for versioned text history
- Content-addressed blob storage for binary attachments
- Admin WebUI for users, device tokens, vaults, invites, runtime settings, and
  sync activity
- Docker, Docker Compose, Caddy, CI, release, and public documentation examples

## Current Features

| Area | Current behavior |
| --- | --- |
| Sync | Multi-user, multi-vault Obsidian sync through the plugin |
| Text history | Text files are committed into per-vault Git history |
| Attachments | Binary files are stored by SHA-256 content hash |
| Conflicts | Conflicting edits are preserved as `.conflict-*` files |
| Exclusions | `.obsidian/`, `.trash/`, and conflict files are not synced |
| Auth | Deployment-key pre-auth plus user passwords and bearer device tokens |
| Devices | Stable plugin device IDs; repeated login replaces the old active token for that device |
| Admin | Dashboard, users, device tokens, vaults, invites, settings, activity, and blob GC |
| Time display | Admin and plugin time display use selectable IANA timezones, defaulting to `Asia/Shanghai` |
| Observability | Structured logs with `json` or `pretty` output and configurable log level |
| Release | Linux amd64, Linux arm64, Windows x64, plugin zip, checksums, and GHCR Docker image |

## Release Assets

GitHub releases publish:

- `pkvsyncd-x86_64-unknown-linux-gnu`
- `pkvsyncd-aarch64-unknown-linux-gnu`
- `pkvsyncd-x86_64-pc-windows-msvc.exe`
- `pkv-sync-plugin.zip`
- `SHA256SUMS`

Docker images are published to:

```bash
docker pull ghcr.io/cyberkurry/pkv-sync:latest
```

Tagged releases also publish `ghcr.io/cyberkurry/pkv-sync:<version>`.

## Quick Start: Docker Compose

Use this path when you want Caddy to request and renew HTTPS certificates.
Caddy needs public ports `80` and `443`; port `80` is used for ACME HTTP-01
validation and redirects.

1. Point DNS at your server:

   ```text
   sync.example.com A    <server IPv4>
   sync.example.com AAAA <server IPv6, optional>
   ```

2. Create a deployment key:

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

3. Create `config.toml` next to `docker-compose.yml`:

   ```toml
   [server]
   bind_addr = "0.0.0.0:6710"
   deployment_key = "k_replace_me"
   public_host = "sync.example.com"

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]

   [logging]
   level = "info"
   format = "json"
   ```

4. Edit `deploy/caddy/Caddyfile` and replace `sync.example.com` with your domain.

5. Start the stack:

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

6. Save the first-run admin password printed in the server logs.

7. Open:

   ```text
   https://sync.example.com/admin/login
   ```

More details are in the [deployment hardening guide](./public-docs/deployment-hardening.md).

## Quick Start: Local Binary

Build from source:

```bash
cargo build -p pkv-sync-server
npm ci --prefix plugin
npm --prefix plugin run build
```

Generate a deployment key:

```bash
./target/debug/pkvsyncd genkey
```

Create `config.toml` from [`config.example.toml`](./config.example.toml), then run:

```bash
./target/debug/pkvsyncd -c config.toml migrate up
./target/debug/pkvsyncd -c config.toml serve
```

For reverse-proxy deployments, bind `pkvsyncd` to localhost:

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

On first start, `pkvsyncd` creates an `admin` account and prints a one-time
password.

## Server CLI

```bash
pkvsyncd genkey
pkvsyncd -c /etc/pkv-sync/config.toml migrate up
pkvsyncd -c /etc/pkv-sync/config.toml serve
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
```

The default config path is `/etc/pkv-sync/config.toml`.

## Obsidian Plugin

Manual install from a release:

1. Download `pkv-sync-plugin.zip`.
2. Extract it into `<vault>/.obsidian/plugins/pkv-sync/`.
3. Enable community plugins in Obsidian.
4. Enable **PKV Sync**.
5. Paste the server share URL from the admin panel:

   ```text
   https://sync.example.com/k_xxx/
   ```

6. Log in, create or select a remote vault, then use automatic sync or **Sync now**.

The plugin stores a stable device ID locally. Logging out and logging back in on
the same device replaces the old active token for that device instead of leaving
multiple active tokens.

## Admin WebUI

Open `/admin/login` on your server. The admin panel currently includes:

- Dashboard with CPU, memory, data-directory disk usage, and human-readable uptime
- User management and password reset
- Device token creation, listing, and revocation
- Vault creation, deletion, metadata reconciliation, and size display
- Invite management
- Runtime settings for server name, timezone, registration mode, and login rate limits
- Activity table with time, user, action, device name, vault name/ID, IP, and User-Agent
- Blob garbage collection trigger

## Configuration Notes

- Default service port: `6710`
- Default timezone: `Asia/Shanghai`
- Default registration mode: `disabled`
- Default max file size: `100 MiB`
- Default text extensions: `md`, `canvas`, `base`, `json`, `txt`, `css`
- `trusted_proxies` controls which reverse proxies may set `X-Forwarded-For`
- `public_host` enables production-style admin cookies and share URL generation

## Documentation

- [Deployment hardening](./public-docs/deployment-hardening.md)
- [Admin manual](./public-docs/admin-manual.md)
- [User manual](./public-docs/user-manual.md)
- [OpenAPI spec](./public-docs/openapi.yaml)
- [Changelog](./CHANGELOG.md)

## Development Checks

```bash
cargo fmt --all -- --check
cargo clippy -p pkv-sync-server --all-targets -- -D warnings
cargo test -p pkv-sync-server
npm --prefix plugin test
npm --prefix plugin run typecheck
npm --prefix plugin run build
cargo build --release -p pkv-sync-server
pwsh -File scripts/ci-smoke.ps1
```

CI runs Rust checks on Linux and Windows, plugin tests/typecheck/build, Docker
build, and release-binary smoke tests.

## License

AGPL-3.0-only. See [LICENSE](./LICENSE).
