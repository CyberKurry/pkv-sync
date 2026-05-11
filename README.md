# PKV Sync

Self-hosted Obsidian vault synchronization with a Rust server, SQLite metadata,
Git-backed text history, content-addressed attachment storage, and a mobile /
desktop Obsidian plugin.

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

English | [简体中文](./README.zh-CN.md)

## Status

PKV Sync is pre-1.0 software. APIs, storage layout, release packaging, and
operational defaults may still change.

PKV Sync does not provide end-to-end encryption. The server can read synced
vault contents and attachments. Use HTTPS, strict account controls, encrypted
disks, encrypted backups, and host-level hardening for real deployments.

## Components

- `pkvsyncd`: server daemon and CLI
- `pkv-sync`: Obsidian plugin for desktop and mobile
- SQLite metadata database
- Per-vault bare Git repositories under the data directory
- SHA-256 content-addressed blob storage for binary attachments
- Admin WebUI for users, device tokens, vaults, invites, settings, activity,
  and cleanup
- Docker, Docker Compose, Caddy, Nginx, Traefik, systemd, CI, and release
  workflow examples

## Current Features

| Area | Current behavior |
| --- | --- |
| Sync model | Multi-user, multi-vault Obsidian sync through authenticated devices |
| Text history | Text files are committed into per-vault Git history |
| History & diff | Obsidian can show per-file history and unified diffs; Admin WebUI can browse files, history, and diffs read-only |
| Single-file restore | Obsidian can restore one historical file version by writing it back locally and letting normal sync push it |
| Attachments | Binary files are stored as SHA-256 blobs and referenced from Git pointer files |
| Conflict handling | Local/remote conflicts are preserved as generated `.conflict-*` files |
| Conflict cleanup | Plugin settings and command palette can list or delete generated conflict files |
| Exclusions | `.obsidian/`, `.trash/`, and generated conflict files are not synced |
| Auth | Deployment-key pre-auth plus username/password login and 90-day bearer device tokens |
| Devices | Stable plugin device IDs; repeated login replaces the old active token for that device |
| Registration | Runtime modes: disabled, invite-only, or open |
| Admin | Responsive dashboard, users, user details, device tokens, vaults, read-only file/history/diff browsing, invites, settings, activity, and blob GC |
| Activity | Push, pull, history, diff, and commit-view activity rows with user/action filters, device name, vault, IP, User-Agent, and details |
| Time display | Admin and plugin timestamps use selectable IANA timezones, defaulting to `Asia/Shanghai` |
| Human-readable values | Admin UI renders time, uptime, durations, sizes, and vault totals in readable units |
| Reliability | Serialized plugin state reads/writes, partial pull progress, idempotent pushes, and per-vault push locks |
| Release | Linux amd64, Linux arm64, Windows x64, plugin zip, checksums, and GHCR Docker image |

## Storage Layout

The configured `[storage].data_dir` contains server-managed state:

```text
data_dir/
  metadata.db        SQLite metadata
  vaults/<vault-id>/ Bare Git repository for each remote vault
  blobs/<sha256>     Content-addressed binary blobs
```

`metadata.db` tracks users, vaults, device tokens, invites, runtime settings,
sync activity, blob references, and idempotency records. Per-vault Git history
is the source of versioned file state; blob files are retained while referenced
and are cleaned by garbage collection after the grace period.

Back up `metadata.db`, `vaults/`, `blobs/`, and `config.toml` together.

## Release Assets

GitHub releases publish:

- `pkvsyncd-x86_64-unknown-linux-gnu`
- `pkvsyncd-aarch64-unknown-linux-gnu`
- `pkvsyncd-x86_64-pc-windows-msvc.exe`
- `pkv-sync-plugin.zip`
- `SHA256SUMS`

Docker images are published to GHCR:

```bash
docker pull ghcr.io/cyberkurry/pkv-sync:latest
docker pull ghcr.io/cyberkurry/pkv-sync:v0.1.9
```

Release Docker images are multi-arch for `linux/amd64` and `linux/arm64`.

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

4. Edit `deploy/caddy/Caddyfile` and replace `sync.example.com` with your
   domain.

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

Create `config.toml` from [`config.example.toml`](./config.example.toml), then
run:

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
password. Store it immediately, then change it from the Admin WebUI or CLI.

## Server CLI

```bash
pkvsyncd genkey
pkvsyncd -c /etc/pkv-sync/config.toml migrate up
pkvsyncd -c /etc/pkv-sync/config.toml serve
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user add alice --admin
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

6. Click **Connect**, then log in or register.
7. Create or select a remote vault.
8. Use automatic sync or **Sync now**.

Plugin settings include:

- Full-width dark settings UI inside Obsidian settings
- Language selector: auto, English, Simplified Chinese
- Timezone selector, defaulting to `Asia/Shanghai`
- Server URL and deployment key parsing from share URLs
- **Change server** from the login/register state without clearing saved input
- Device name editing and stable local device ID storage
- Login, registration, logout, remote vault creation, and vault selection
- Manual sync button
- Last successful sync shown as relative time, with an expandable exact
  `YYYY/MM/DD HH:MM:SS` timestamp
- Conflict file count and one-click deletion of generated conflict files
- Device list with current device marker
- History and diff UI toggle

Command palette actions:

- Show sync status
- Refresh account info
- Manual sync now
- View sync status details
- Show file history
- Show vault history
- List conflict files
- Delete conflict files

Sync behavior:

- Pushes local changes after the debounce interval
- Polls remote changes periodically
- Syncs after relevant vault file events and on window blur
- Uses the server-provided text extension list after connecting
- Verifies downloaded binary blob hashes before writing them locally
- Stores plugin settings and sync indexes through a serialized data store
- Records partial pull progress if a write fails midway, reducing duplicate
  conflict files on retry
- Restores a selected file version by reading historical content from the
  server, writing it to the local vault, and letting the existing sync engine
  push it as a normal change

Device tokens expire after 90 days. Logging in again on the same device replaces
the previous active token for that device instead of leaving multiple active
tokens.

## Admin WebUI

Open `/admin/login` on your server. The Admin WebUI includes:

- Dashboard with CPU, memory, data-directory disk usage, uptime, users, vaults,
  and recent activity
- Responsive sidebar with mobile drawer navigation and bundled Lucide icons
- User list, user creation, user detail pages, password reset, active/admin
  controls, and per-user token management
- Global device token page for listing, creating, and revoking tokens
- Vault cards with owner, file count, size, last sync, reconcile, and delete
  actions
- Read-only vault file browser with file preview, per-file history timeline, and
  unified diff viewer. Admin WebUI does not provide restore, revert, or rollback
  controls.
- Invite creation, expiration display, and deletion for unused invites
- Runtime settings grouped as General, Security, Sync & Storage, and Network
- Login rate-limit settings
- Max file size and supported text extension settings
- Blob garbage collection trigger
- Activity log with real filters for user and action
- English and Simplified Chinese admin language selection

Safeguards include last-admin protection, self-disable/self-delete protection,
username validation, password hashing with Argon2id, 90-day device-token
expiration, token revocation, CSRF checks for admin forms, and deployment-key
pre-auth for API routes.

## Configuration Notes

Static `config.toml` fields:

- `server.bind_addr`: default service listener, commonly `127.0.0.1:6710` behind
  a reverse proxy or `0.0.0.0:6710` in Docker Compose
- `server.deployment_key`: generated by `pkvsyncd genkey`
- `server.public_host`: optional host used for HTTPS share URL generation and
  production-style admin cookies
- `storage.data_dir`: data root containing `metadata.db`, `vaults/`, and `blobs/`
- `storage.db_path`: SQLite database path
- `network.trusted_proxies`: CIDRs allowed to set `X-Forwarded-For`
- `logging.level`: tracing filter such as `info` or `debug`
- `logging.format`: `json` or `pretty`

Runtime settings stored in SQLite and editable from Admin WebUI:

- Server name
- Timezone, default `Asia/Shanghai`
- Registration mode: `disabled`, `invite_only`, or `open`
- Login failure threshold, window, and lock duration
- Maximum file size, default `100 MiB`
- Supported text extensions, default `md`, `canvas`, `base`, `json`, `txt`, `css`
- History UI and diff endpoint feature flags, both enabled by default

## HTTP API

All `/api/*` routes require the deployment key header. Authenticated routes also
require a bearer device token.

Main route groups:

- `GET /api/health`
- `GET /api/config`
- `POST /api/auth/login`
- `POST /api/auth/register`
- `GET /api/me`
- `POST /api/me/password`
- `POST /api/me/logout`
- `GET /api/me/tokens`
- `DELETE /api/me/tokens/:id`
- `GET /api/vaults`
- `POST /api/vaults`
- `DELETE /api/vaults/:id`
- `POST /api/vaults/:id/upload/check`
- `POST /api/vaults/:id/upload/blob`
- `GET /api/vaults/:id/state`
- `POST /api/vaults/:id/push`
- `GET /api/vaults/:id/pull`
- `GET /api/vaults/:id/commits`
- `GET /api/vaults/:id/commits/:commit`
- `GET /api/vaults/:id/history?path=`
- `GET /api/vaults/:id/diff?from=&to=&path=`
- `GET /api/vaults/:id/files/*path`
- Admin API routes under `/api/admin/*`

See the [OpenAPI spec](./public-docs/openapi.yaml) for schemas.

## Operations

- Keep `config.toml`, `metadata.db`, `vaults/`, and `blobs/` in the same backup
  set.
- Run behind HTTPS. Example reverse-proxy configs are provided for Caddy, Nginx,
  and Traefik.
- If using a reverse proxy, set `trusted_proxies` to only the proxy network.
- Watch logs for repeated `401`, `403`, `409`, and `429` responses.
- Run blob garbage collection after large attachment deletions.
- Use vault metadata reconciliation if file counts, sizes, or blob references
  drift after interrupted operations.
- Keep release assets, Docker images, plugin package, changelog, and version
  numbers aligned when releasing.

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
npm --prefix plugin run package
cargo build --release -p pkv-sync-server
pwsh -File scripts/ci-smoke.ps1
```

CI runs Rust formatting, Clippy, and tests on Linux and Windows; plugin
tests/typecheck/build/package/audit; Docker build; and release-binary smoke
tests.

Release CI additionally builds Linux amd64, Linux arm64, Windows x64, the
plugin package, the multi-arch Docker image, checksums, and the GitHub release.

## License

AGPL-3.0-only. See [LICENSE](./LICENSE).
