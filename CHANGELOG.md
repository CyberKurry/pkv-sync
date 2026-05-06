# Changelog

All notable changes to PKV Sync will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to semantic versioning after v1.0.0.

## [Unreleased]

## [0.1.3] - 2026-05-06

### Changed

- Redesign the Admin WebUI around the new dark dashboard shell, sidebar navigation, metric cards, tables, forms, and login screen.
- Add a global Admin WebUI device-token page so admins can review, create, and revoke device tokens across users.
- Improve Admin WebUI dashboard, vault, invite, activity, user, device, and settings pages for deployment-time inspection.

## [0.1.2] - 2026-05-05

### Added

- Add timezone selectors to the admin WebUI and Obsidian plugin, defaulting to Asia/Shanghai.
- Show the last successful sync time in the Obsidian plugin using the plugin-selected timezone.
- Persist a stable plugin device ID and include it in login/register requests.

### Changed

- Render human-readable timestamps without appending the timezone suffix.
- Use desktop hostnames as default device names when available, with clearer mobile fallback names.
- Replace prior active tokens for the same user and device ID when a device logs in again.

### Fixed

- Prevent repeated logout/login cycles from leaving multiple active tokens for the same device.

## [0.1.1] - 2026-05-03

### Changed

- Admin pages now render stored Unix timestamps as timezone-aware human-readable times using the runtime-configured IANA timezone.

### Fixed

- Record client IP and User-Agent metadata for vault push activity so the admin activity table is no longer blank for new push rows.

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
