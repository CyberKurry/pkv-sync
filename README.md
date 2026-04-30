# PKV Sync

Self-hosted Obsidian vault sync with server-side version history.

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

English | [简体中文](./README.zh-CN.md)

## What It Is

PKV Sync is a small self-hosted sync stack for Obsidian:

- `pkvsyncd`: Rust server daemon with a SQLite metadata database, Git-backed vault history, and an HTTP API
- `pkv-sync`: Obsidian plugin for desktop and mobile clients

It is intended for a single trusted server used by yourself, family, or a small group.

## Status

Pre-release. APIs, storage layout, and release packaging may change before v1.0.

PKV Sync does not provide end-to-end encryption in the current design. Use HTTPS,
disk encryption, strict access control, and encrypted backups for real deployments.

## Features

| Area | Current behavior |
| --- | --- |
| Vault sync | Multi-user, multi-vault sync through the Obsidian plugin |
| Version history | Text files are committed into per-vault Git history on the server |
| Attachments | Binary files are stored in a content-addressed blob store |
| Conflicts | Conflicting local edits are preserved as `.conflict-*` files |
| Admin | First-run admin bootstrap, admin web panel, user and invite management |
| Auth | Deployment-key pre-auth plus bearer device tokens |
| Storage | SQLite metadata, local filesystem data directory |
| Operations | systemd, reverse proxy, Docker Compose, and release packaging support |

## Quick Start (Development)

Build the server and plugin:

```bash
cargo build -p pkv-sync-server
npm install --prefix plugin
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

On first start, `pkvsyncd` creates an `admin` account and prints a one-time password.

## Deployment Paths

| Path | Use when |
| --- | --- |
| Binary + systemd + reverse proxy | You want direct control over the host and data directory |
| Docker Compose + Caddy | You want a simple containerized deployment |
| Existing reverse proxy | You already run Caddy, Nginx, Traefik, or another TLS terminator |

Start with the [deployment hardening guide](./public-docs/deployment-hardening.md)
before exposing a server to the internet.

## Obsidian Plugin

Manual install from a release:

1. Download `pkv-sync-plugin.zip`.
2. Extract it into `<vault>/.obsidian/plugins/pkv-sync/`.
3. Enable community plugins in Obsidian.
4. Enable **PKV Sync** and paste the server share URL from your admin.

The share URL has this shape:

```text
https://sync.example.com/k_xxx/
```

## Documentation

- [Deployment hardening](./public-docs/deployment-hardening.md)
- [Admin manual](./public-docs/admin-manual.md)
- [User manual](./public-docs/user-manual.md)
- [OpenAPI spec](./public-docs/openapi.yaml)
- [Changelog](./CHANGELOG.md)

## Development Checks

```bash
cargo fmt --check
cargo clippy -p pkv-sync-server -- -D warnings
cargo test -p pkv-sync-server
npm --prefix plugin test
npm --prefix plugin run typecheck
npm --prefix plugin run build
```

## License

AGPL-3.0-only. See [LICENSE](./LICENSE).
