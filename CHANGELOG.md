# Changelog

All notable changes to PKV Sync will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to semantic versioning after v1.0.0.

## [Unreleased]

## [0.3.2] - 2026-05-18

### Fixed

- Vault row in settings tab: the select button and delete button now sit on the same row inside an action group instead of the delete button wrapping to its own line.
- Vault delete button style now matches the select button (consistent border, radius, size).

## [0.3.1] - 2026-05-17

### Security

- **CSRF**: admin CSRF check now fails closed when `[server].public_host`
  is not configured, instead of falling back to the request `Host` header.
  Operators who have not set `public_host` see admin POSTs rejected with a
  log line pointing at the missing setting.
- **Login rate limit**: new atomic `try_acquire` reservation closes the
  race window where a burst of concurrent guesses could all pass the
  pre-check and consume CPU on argon2 before any of them recorded a
  failure. In-flight reservations now count toward the threshold so the
  `(threshold+1)`th request is rejected before reaching password verify.
- **Login enumeration**: a login attempt against a disabled account now
  returns 401 with the same "invalid credentials" message as a wrong
  password, removing the 401 / 403 side channel that previously leaked
  account state. The same change routes disabled-account attempts through
  the rate-limit budget.
- **Register rate limit**: register failures are now classified into abuse
  signals (invite probing, mode probing, username enumeration via
  CONFLICT) which consume budget, vs. honest validation typos
  (username too short, weak password) which do not. The handler
  previously consumed budget on UNAUTHORIZED only — a status the register
  flow never returned — so abuse was unlimited.

### Fixed

- SSE `source_device_id` carries the stable per-device id from the token
  instead of the token row id, so client-side echo filtering works and a
  device no longer receives its own pushes back as SSE events.
- The SSE broadcast for a push now fires immediately after `git commit`
  succeeds, before idempotency cache and activity log writes. This keeps
  the sub-second latency budget honest.
- Plugin `applyInlineText` refuses to silently overwrite a local file
  that has diverged from the sync index (unsynced local edits). The
  inline apply throws and the engine falls back to a full pull, which
  preserves local content as a `.conflict-*` file.
- Plugin `applyInlineText`, `applyDelete`, and `advanceIndexHead` now run
  inside an atomic `IndexPersistence.updateIndex` transaction backed by
  the serialized plugin data store. The SSE event handler additionally
  serialises events through a promise chain so two fast-succession
  events cannot interleave their reads and writes.
- `push_debounce_ms` is now exposed on `/api/config` and mirrored into
  plugin settings so admin tuning of the debounce window actually
  reaches connected clients.
- `/api/config` capabilities.git_smart_http now ANDs the runtime toggle
  with whether the `git` binary is available on the server, so clients
  do not advertise Git clone when the request would 503.
- Admin form rejects `inline_content_max_bytes` values above 64 KiB.
- `git-upload-pack` request body is capped at 10 MiB to bound memory use.
- Glob compile failures during sync now log a `tracing::warn` instead of
  silently falling back to "match nothing".

### Changed

- `AuthenticatedUser` now carries `device_id` alongside `token_id`; bearer
  and basic-auth extractors both populate it from the token row.
- `IndexPersistence` interface gains an `updateIndex(updater)` method for
  atomic read-modify-write on the sync index. The plugin data store's
  serialisation primitive is now the canonical path for index updates.

## [0.3.0] - 2026-05-17

### Added

- SSE push notifications: the server broadcasts vault change events to connected plugins in real time via `GET /api/vaults/:id/events`. Small text changes (≤ 8 KB) are delivered inline, eliminating the need for a separate pull round-trip.
- Plugin SSE subscription with inline apply: the Obsidian plugin opens an SSE stream on startup, writes inline text content and deletes directly to disk, and falls back to a full pull for blob or large-text changes. Self-originated events are filtered by device ID.
- Git smart HTTP (read-only): clone any PKV Sync vault using `git clone` over HTTP. Auth uses the standard `Authorization: Basic` header bridged to the PKV Sync token system. Disabled by default via the `enable_git_smart_http` runtime flag; returns 503 when git is not found on the server PATH.
- `pkvsyncd materialize` CLI subcommand: walks the bare git tree for a vault and writes a working copy to an output directory, resolving blob pointer JSONs by copying the actual binary data from the sharded blob store.
- Admin WebUI settings for SSE heartbeat interval, push debounce window, inline content size limit, and Git smart HTTP toggle.
- `ServerCapabilities.git_smart_http` field in the public config response so clients can discover whether Git clone is available.

### Changed

- Default push debounce reduced from 2000 ms to 250 ms for faster SSE event propagation.
- SSE heartbeat (default 30 s) keeps idle connections alive through proxies and load balancers.

## [0.2.1] - 2026-05-17

### Fixed

- Add CSS for `DeleteVaultModal`, `ConflictsListModal`, and `ConflictResolveModal`; previous v0.2.0 ship rendered these modals unstyled.
- Conflict resolution list now reopens automatically after each resolve so multiple conflicts can be cleared without re-invoking the command.
- Conflict resolve modal now renders an actual LCS-based line diff with add/del/modify highlighting instead of two raw side-by-side text columns.
- Binary file detection in the conflict resolve modal: extension-based check plus NUL-byte scan, fixing the prior reliance on `vault.read` throwing.
- Server now emits `tracing::warn` instead of silently falling back when `extra_exclude_globs` fails to compile (previously a malformed admin glob would disable all filtering with no log).
- Added end-to-end server tests for `extra_exclude_globs` push rejection (text + nested paths) and non-matching path acceptance.

## [0.2.0] - 2026-05-17

### Added

- Delete vault entry in the Obsidian plugin settings, with typed-confirm protection (closes #3).
- Runtime-configurable extra exclude globs (admin Sync &amp; Storage panel).
- Conflict file resolution UI: command palette "Resolve conflict files" lists `.conflict-*` files, shows diff vs the original, and offers "Keep local" / "Accept remote" / "Later".
- Obsidian community plugin store submission materials and `versions.json`.

## [0.1.12] - 2026-05-12

### Security

- Stop registration validation errors from consuming the login rate-limit budget.
- Rotate existing Admin WebUI sessions after a successful admin login.
- Use generic Admin WebUI login errors to reduce account-state disclosure.
- Reject non-loopback HTTP server URLs in the Obsidian plugin while preserving local development URLs.
- Add password, upload-check, and file-history scan limits to reduce avoidable resource abuse.
- Remove vault storage when an administrator deletes a user through the Admin API.
- Harden Admin WebUI language redirect targets to stay under the admin path boundary.

### Fixed

- Rebuilt the Obsidian plugin package with the latest sync safety changes.

## [0.1.11] - 2026-05-12

### Security

- Mask client IP addresses in the Admin WebUI activity log while retaining enough prefix/suffix detail for troubleshooting.
- Limit the Admin WebUI activity log to the latest 30 rows; deeper log inspection now stays server-side.

### Changed

- Add missing Admin WebUI action and settings icons across the admin templates.
- Default the Admin WebUI to light mode and keep the existing dark palette for browser dark-mode preference.
- Add top safe-area padding for the Obsidian plugin settings UI on mobile.

## [0.1.10] - 2026-05-12

### Security

- Harden Obsidian plugin sync paths so remote pull data cannot write `.obsidian`, `.trash`, `.git`, absolute paths, drive-letter paths, or traversal paths into a vault.
- Harden Admin WebUI CSRF origin checks by using the configured public host when present and only trusting `X-Forwarded-Proto` from configured trusted proxies.
- Prune stale login rate-limit entries so expired failures and locks do not accumulate indefinitely.

## [0.1.9] - 2026-05-11

### Added

- Per-file commit history endpoint `GET /api/vaults/:id/history?path=`.
- Unified diff endpoint `GET /api/vaults/:id/diff?from=&to=&path=`.
- Obsidian plugin file history and diff modals from the command palette and file menu.
- Obsidian plugin single-file restore from history/diff views. Historical content is written back to the local file and pushed by the existing sync engine; no new server write endpoint is added.
- Admin WebUI read-only vault file browser, per-file history timeline, and unified diff viewer. Admin still has no restore, revert, or rollback UI.
- Runtime config flags `enable_history_ui` and `enable_diff_endpoint`.

### Changed

- BREAKING: `GET /api/vaults/:id/commits/:commit` now returns parent-diff `changes[]` instead of the full commit tree listing. This remains within the pre-1.0 API compatibility window.

## [0.1.8] - 2026-05-09

### Changed

- Let the Obsidian plugin settings page fill the Obsidian settings pane, add a back path from login to server configuration, and add one-click conflict-file cleanup.
- Make Admin WebUI user filtering and invite creation interactive instead of decorative, and remove the inert dashboard search box.
- Refresh the public OpenAPI document to match the current API surface.

### Fixed

- Avoid creating full-vault conflict files when a user reconnects an existing local vault copy to an identical remote vault.
- Show newly created Admin WebUI invites after creation and validate human-readable invite expiry input.

## [0.1.7] - 2026-05-09

### Changed

- Redesign the Obsidian plugin settings UI to match the dark card-based design for connection, login/register, vault selection, manual sync, and device states.
- Show the Obsidian plugin's last successful sync as compact relative time with an expandable exact timestamp.

## [0.1.6] - 2026-05-08

### Security

- Validate admin token-revocation routes against the URL user while preserving administrator access to revoke tokens for any user.
- Reject oversized push change sets before server-side processing to reduce memory and CPU abuse risk.

### Changed

- Record pull operations in the sync activity log and make the Admin WebUI activity filters actually filter by user and action.
- Let the Obsidian plugin use the server-provided text extension list after connecting.

### Fixed

- Serialize plugin sync-index reads behind pending data writes so sync decisions do not use stale plugin state.
- Keep normal filenames such as `my.conflict-resolution-notes.md` eligible for sync while still excluding generated conflict files.
- Normalize corrupted numeric plugin settings back to safe defaults.
- Write login rate-limit runtime configuration keys in one transaction.
- Cleanly coordinate vault deletion with per-vault push locks.
- Avoid panic-prone JSON serialization unwraps in user and vault API responses.

## [0.1.5] - 2026-05-07

### Security

- Add a 90-day expiration time to API bearer device tokens, including a migration for existing active tokens.
- Keep the "last admin" protection atomic for Admin WebUI and admin API role changes.

### Changed

- Redesign the Admin WebUI user detail page so it uses the same responsive shell, panels, action cards, icons, and token table style as the rest of the admin interface.
- Make the plugin unload path cancel timers and invalidate the current sync engine instead of starting a new unload-time sync.

### Fixed

- Serialize Obsidian plugin data writes so settings and sync index updates cannot silently overwrite each other.
- Preserve partial pull progress after interrupted file writes, avoiding duplicate conflict files on the next retry.
- Delete the backing vault Git repository and clear its push lock when a vault is removed.
- Add invite foreign-key delete actions so users connected to invites can be deleted without breaking referential integrity.
- Keep the Admin WebUI user detail layout from overflowing on mobile and remove misleading timezone suffixes from its regression fixtures.

## [0.1.4] - 2026-05-06

### Changed

- Make the Admin WebUI shell fully fluid instead of constraining it to the design mockup canvas size.
- Replace hand-drawn Admin WebUI icons with a bundled Lucide icon sprite.
- Collapse the Admin WebUI sidebar into a hamburger drawer on mobile viewports.
- Reorder Admin WebUI settings sections as General, Security, Sync & Storage, and Network.
- Bump `sqlx` to 0.8.1.

### Fixed

- Report CPU cores from container cgroup CPU quota when available, avoiding misleading host-core counts in Docker deployments.

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
