# Changelog

All notable changes to PKV Sync will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to semantic versioning after v1.0.0.

## [Unreleased]

## [0.1.0] - 2026-04-30

### Added

- Initial Rust sync server with token auth, admin sessions, SQLite metadata, Git-backed vault history, and content-addressed blob storage.
- Obsidian plugin with login, vault creation, manual sync, pull/push conflict handling, and English / Simplified Chinese UI text.
- Admin web panel for runtime settings, users, device tokens, vaults, activity, and cleanup visibility.
- Scheduled cleanup for expired admin sessions, revoked tokens, old activity, idempotency cache entries, and unreferenced blobs.
- Runtime-configurable maximum file size and text extension handling in server config responses.
- Docker, Docker Compose, Caddy deployment examples, public docs, OpenAPI docs, and GitHub Actions CI/release workflows.

### Changed

- Use port 6710 as the default service port.
- Apply upload and push size limits from runtime configuration instead of a hard-coded global body limit.
- Use the admin cookie Secure attribute only for deployments with a configured public host, keeping local HTTP admin login usable.
- Keep recent unreferenced blobs during garbage collection to avoid deleting in-flight uploads.
- Prevent generated conflict files from being re-synced by the Obsidian plugin.

### Fixed

- Hardened sync correctness around blob pointer detection, idempotency keys, path filtering, local delete conflicts, and blob size accounting.
- Keep push statistics, blob references, and sync activity updates in one database transaction.
- Convert blocking task panics in Git/history operations into internal errors instead of panicking the server.
- Keep cleanup tests portable across Linux and Windows by preserving temporary database directories for the full test lifetime.
