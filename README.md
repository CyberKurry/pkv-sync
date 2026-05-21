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

PKV Sync does **not** provide end-to-end encryption. The server can read
synced vault contents and attachments. Use HTTPS, strict account controls,
encrypted disks, encrypted backups, and host-level hardening for real
deployments — see the [deployment hardening guide](./public-docs/deployment-hardening.md).

## Highlights

- **Multi-user, multi-vault** Obsidian sync through authenticated devices, with
  per-vault push locks and idempotent pushes.
- **Real-time push** via Server-Sent Events: small text changes (≤ 8 KiB) are
  delivered inline in the event so the plugin applies them without a separate
  pull. Sub-second end-to-end target on a healthy network. Polling stays as a
  safety-net fallback.
- **Git-native**: each vault is a bare git repository on disk. Per-file history,
  unified diff, and single-file restore are exposed in both the Obsidian plugin
  and the admin panel. Optional read-only `git clone https://_:<token>@host/git/<vault>`
  for offline browsing or external mirroring.
- **AI-readable vaults**: `pkvsyncd mcp` exposes read-only vault tools through
  MCP over stdio or a stateless Streamable HTTP endpoint.
- **Selective `.obsidian` sync**: new vaults get a starter allowlist for
  themes, snippets, hotkeys, app preferences, appearance, and enabled plugin
  lists. Plugin code and plugin settings stay opt-in.
- **Conflict-safe**: SSE inline apply refuses to overwrite a locally-modified
  file; conflicts surface as generated `.conflict-*` files that preserve the
  original extension and can be resolved from the plugin command palette with a
  real LCS-based line diff and one-click "keep local" / "accept remote"
  buttons.
- **Admin panel** for users, device tokens, vaults, invites, runtime settings,
  activity log, and blob garbage collection. Responsive, English + 简体中文.
- **Security**: Argon2id password hashing, atomic per-IP login rate limiter
  with burst protection, CSRF fail-closed when `public_host` is unset, unified
  "invalid credentials" response across wrong-password and disabled-account
  cases, 90-day bearer device tokens with rotation on re-login.
- **Boring on purpose**: single binary, single SQLite metadata DB, one bare
  git repo per vault, one content-addressed blob per attachment. No cluster,
  no MySQL/PostgreSQL backend, no S3 dependency.
- Linux amd64 / arm64, Windows x64 binaries plus a multi-arch GHCR Docker image.

See the [admin manual](./public-docs/admin-manual.md) and
[user manual](./public-docs/user-manual.md) for full operational and end-user
walkthroughs.

## Storage Layout

```text
data_dir/
  metadata.db        SQLite metadata
  vaults/<vault-id>/ Bare Git repository for each remote vault
  blobs/<sha256>     Content-addressed binary blobs
```

`metadata.db` tracks users, vaults, device tokens, invites, runtime settings,
sync activity, blob references, and idempotency records. Per-vault Git history
is the source of truth for versioned file state; blob files are retained while
referenced and cleaned by garbage collection after the grace period. Use
`pkvsyncd backup` to snapshot the data root and matching `config.toml`.

## Release Assets

GitHub releases publish:

- `pkvsyncd-x86_64-unknown-linux-gnu`
- `pkvsyncd-aarch64-unknown-linux-gnu`
- `pkvsyncd-x86_64-pc-windows-msvc.exe`
- `pkv-sync-plugin.zip`
- `SHA256SUMS`

Docker images are published multi-arch (`linux/amd64`, `linux/arm64`) to GHCR:

```bash
docker pull ghcr.io/cyberkurry/pkv-sync:latest
docker pull ghcr.io/cyberkurry/pkv-sync:v0.5.1
```

## Quick Start: Docker Compose

This is the recommended path. Caddy in `deploy/caddy/` requests and renews
HTTPS certificates via Let's Encrypt; PKV Sync listens on `127.0.0.1:6710`
inside the compose network and never sees plain HTTP from the public internet.

**Requirements**: a public DNS A/AAAA record pointing at the server, ports `80`
and `443` reachable from the internet (port 80 is needed for ACME HTTP-01
validation and HTTP→HTTPS redirects).

1. **Point DNS at the server**

   ```text
   sync.example.com A    <server IPv4>
   sync.example.com AAAA <server IPv6, optional>
   ```

2. **Generate a deployment key**

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

3. **Create `config.toml` next to `docker-compose.yml`**

   ```toml
   [server]
   bind_addr     = "0.0.0.0:6710"
   deployment_key = "k_replace_me_with_genkey_output"
   public_host   = "sync.example.com"   # required for admin POST

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker bridge network

   [logging]
   level  = "info"
   format = "json"
   ```

   `public_host` is **load-bearing**: when unset, the admin CSRF check fails
   closed and every admin POST is rejected (see deployment hardening guide).

4. **Edit `deploy/caddy/Caddyfile`** and replace `sync.example.com` with your
   domain. The compose file mounts this Caddyfile and a writable
   `caddy_data` volume for Let's Encrypt certificates.

5. **Start the stack**

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

   On first start, PKV Sync creates an `admin` account and prints a one-time
   password to stderr — **save it immediately**. The line is shaped as:

   ```text
   FIRST-RUN ADMIN CREATED
    username: admin
    password: <save this now>
   ```

6. **Sign in**

   Open `https://sync.example.com/admin/login`, sign in as `admin`, change the
   password, and create your first user account from **Users → New**.

**Where things live**

- Server data: `./data` on the host, bind-mounted to `/var/lib/pkv-sync` in
  the container. Snapshot it with `pkvsyncd backup` before maintenance.
- Caddy certificates: `caddy_data` named volume.
- Logs: `docker compose logs pkv-sync` (JSON formatted by default).

**Updating**

```bash
docker compose pull
docker compose up -d
```

Database migrations are append-only and run automatically on start. To roll
back, restore the data directory from a backup.

**Production hardening** — read the
[deployment hardening guide](./public-docs/deployment-hardening.md) for:
reverse-proxy specifics (Caddy / Nginx / Traefik), `trusted_proxies` tuning,
`public_host` semantics, runtime CSRF behaviour, backups, disk encryption,
and token hygiene.

## Server CLI

```bash
pkvsyncd genkey                                      # generate a deployment key
pkvsyncd -c /etc/pkv-sync/config.toml migrate up     # apply database migrations
pkvsyncd -c /etc/pkv-sync/config.toml serve          # run the HTTP server
pkvsyncd -c /etc/pkv-sync/config.toml user add alice [--admin]
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
pkvsyncd -c /etc/pkv-sync/config.toml materialize <vault-id> --output <dir>
pkvsyncd -c /etc/pkv-sync/config.toml backup --output <dir> [--data-dir <dir>] [--gzip]
pkvsyncd -c /etc/pkv-sync/config.toml restore --input <backup-dir> --data-dir <dir> [--force]
pkvsyncd -c /etc/pkv-sync/config.toml verify [--data-dir <dir>] [--no-fail]
```

Default config path: `/etc/pkv-sync/config.toml`.

`materialize` walks a vault's bare git tree and resolves blob pointer files
into the actual binary content — useful for `git clone` users or offline
inspection.

`backup` snapshots `metadata.db` with `VACUUM INTO`, copies `vaults/`,
`blobs/`, and `config.toml` when present, and writes a `MANIFEST.json` with
the pkvsyncd version plus component hashes, sizes, and counts. Use `--data-dir`
for offline checks against a stopped server's data root, and `--gzip` when a
single archive is more convenient than a directory.

`restore` checks the manifest and component hashes before copying data back
into the `--data-dir` target. Use `--force` to clear a non-empty target first,
then it runs `verify` on the restored tree.

`verify` checks referenced blob files against their SHA-256 names, reports
orphan blobs, and validates vault git repositories with `git2`. It exits
non-zero on missing, corrupt, or git errors unless `--no-fail` is set.

## Obsidian Plugin

Install from the bundled release zip (`pkv-sync-plugin.zip`) into
`<vault>/.obsidian/plugins/pkv-sync/`, enable community plugins, and turn on
**PKV Sync** in Obsidian settings. Paste the server share URL
(`https://sync.example.com/k_xxx/`) from the admin panel, click **Connect**,
then log in or register and pick a remote vault.

**Local files are the source of truth.** The plugin reads from and writes to
your normal Obsidian vault on disk — no opaque storage layer, no proxy
filesystem. Plugin settings and the sync index live in Obsidian's
`<vault>/.obsidian/plugins/pkv-sync/data.json`.

`data.json` contains the active bearer device token and deployment key. Treat
it as sensitive: do not publish it, sync it to untrusted locations, or keep it
in plaintext backups. If it may have leaked, revoke the device token and
connect again.

Device tokens expire after 90 days. Logging in again on the same device
replaces the previous active token; concurrent stale tokens are not kept.

See the [user manual](./public-docs/user-manual.md) for the full feature
walkthrough (command palette, history & diff modals, conflict resolution,
selective sync rules, device management, language and timezone).

## Configuration

Static `config.toml` (read at startup):

| Field | Purpose |
| --- | --- |
| `server.bind_addr` | Where the daemon listens. `127.0.0.1:6710` behind a reverse proxy; `0.0.0.0:6710` in Docker Compose. |
| `server.deployment_key` | Generated by `pkvsyncd genkey`; sent by clients in the `X-PKVSync-Deployment-Key` header. |
| `server.public_host` | Externally-visible hostname (and port if non-standard). **Required for admin POSTs** — see deployment hardening guide. |
| `storage.data_dir` | Data root containing `metadata.db`, `vaults/`, and `blobs/`. |
| `storage.db_path` | SQLite database path (usually `<data_dir>/metadata.db`). |
| `network.trusted_proxies` | CIDRs allowed to set `X-Forwarded-For` / `X-Forwarded-Proto`. |
| `logging.level` | tracing filter such as `info`, `debug`. |
| `logging.format` | `json` or `pretty`. |

Runtime settings (registration mode, login rate limits, max file size, text
extensions, push debounce, inline SSE content cap, SSE heartbeat, Git smart
HTTP toggle, extra exclude globs, history/diff feature flags) are edited
from the Admin panel — see the
[admin manual](./public-docs/admin-manual.md#runtime-settings).

## HTTP API

All `/api/*` routes require the deployment key header; authenticated routes
also require a bearer device token. Authenticated sync API routes are
fixed-window rate limited at 600 requests per 60 seconds per route, method,
client IP, and bearer token. SSE clients can replay missed commits with
`Last-Event-ID`; replay is capped and falls back to a `lagged` event when the
client should pull to catch up.

`/metrics` exposes Prometheus metrics only when the `enable_metrics` runtime
setting is true. The route is behind the deployment key middleware and plugin
User-Agent guard, and it requires an admin bearer token. See the
[OpenAPI specification](./public-docs/openapi.yaml) for the full route table,
request / response schemas, metrics endpoint, and SSE event payload format.

## Operations

- Snapshot with `pkvsyncd backup --output /var/backups/pkv/<date>`.
- Periodically run `pkvsyncd verify` to catch SHA drift or orphan blobs.
- Restore with `pkvsyncd restore --input /var/backups/pkv/<date> --data-dir
  /var/lib/pkv-sync`.
- Use `pkvsyncd restore --force` only when the destination data directory can
  be cleared first.
- Run behind HTTPS; restrict `[network].trusted_proxies` to the actual proxy
  CIDRs.
- Watch logs for repeated `401`, `403`, `409`, and `429` responses.
- Run blob garbage collection from the admin panel after large attachment
  deletions.
- Use vault metadata reconciliation if file counts, sizes, or blob references
  drift after interrupted operations.

## Documentation

- [Deployment hardening](./public-docs/deployment-hardening.md)
- [Admin manual](./public-docs/admin-manual.md)
- [User manual](./public-docs/user-manual.md)
- [OpenAPI spec](./public-docs/openapi.yaml)
- [Changelog](./CHANGELOG.md)

## Development Checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm --prefix plugin exec vitest run
npm --prefix plugin run typecheck
npm --prefix plugin run build
npm --prefix plugin run package
cargo build --release -p pkv-sync-server
pwsh -File scripts/ci-smoke.ps1
```

CI runs Rust formatting, Clippy, and tests on Linux and Windows; plugin
tests/typecheck/build/package/audit; Docker build; and release-binary smoke
tests. Release CI additionally builds Linux amd64 / arm64, Windows x64, the
plugin package, the multi-arch Docker image, checksums, and the GitHub
release.

## License

AGPL-3.0-only. See [LICENSE](./LICENSE).
