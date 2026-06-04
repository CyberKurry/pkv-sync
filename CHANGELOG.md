# Changelog

All notable changes to PKV Sync will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to semantic versioning starting at v1.0.0.

## [Unreleased]

## [1.0.10] - 2026-06-05

### Fixed

- Switch Rust coverage collection in CI to tarpaulin's LLVM engine and refresh
  the documented coverage baseline after the default ptrace engine segfaulted
  in otherwise passing Ubuntu admin integration tests.
- Align Admin WebUI settings section headers with the rest of the Working Paper
  panel style.
- Add Admin WebUI confirmation dialogs for vault deletion, user disable, and
  admin demotion; rejected self-disable and last-admin demotion attempts now
  return localized Admin HTML feedback instead of a bare JSON error page.
- Normalize vault/file detail headers across Admin file browsing, file preview,
  diff, and per-vault settings pages.

## [1.0.9] - 2026-06-04

### Fixed

- Harden findings from the v1.0.9 security audit, including reviewed rate-limit
  boundaries, account-state handling, and release metadata alignment.
- Keep package, OpenAPI, and public release metadata on version `1.0.10` for
  the patch line.

## [1.0.8] - 2026-06-03

### Fixed

- Reject file/directory path conflicts in Git tree construction so pushes
  cannot silently replace a file with a directory prefix, or the reverse.
- Check vault ownership before acquiring the per-vault push lock, and filter
  auto-merge conflict paths through the same push path policy as ordinary
  changes.
- Protect pending blob uploads from garbage collection and avoid reading whole
  blob files only to compute freed size.
- Hide disabled-account state from bearer authentication, rate-limit failed
  bearer/MCP/Git HTTP/admin/account attempts, and cap device token lifetime at
  365 days.
- Precheck MCP write content size and validate vault names, device names,
  invite expiry, and unique constraint handling more strictly.
- Secure expired session cookies and expire Admin sessions after password
  resets.

## [1.0.7] - 2026-06-02

### Fixed

- Revalidate main vault SSE streams while they are open so revoked tokens or
  disabled accounts stop receiving live commit events and inline text content.
- Preserve vault push-lock identity during deletion while other guards or
  waiters still hold the lock.
- Wake the runtime update-check loop immediately when Admin WebUI settings
  change.

## [1.0.6] - 2026-05-28

### Added

- Add opt-in embedded MCP HTTP transport for `pkvsyncd serve`; set
  `[mcp].embed_in_serve = true` to mount `/mcp` on the main server port while
  keeping the same deployment-key and bearer-token checks.

### Changed

- Make update-check enabled state and interval runtime-editable from the Admin
  WebUI. `config.toml` still seeds fresh databases, while `[update_check].repo`
  remains a static deployment setting.
- Omit `config.toml` from CLI backups by default; use `--include-config` only
  when the backup location protects deployment keys and local secrets.

### Fixed

- Preserve committed push metadata and blob references when post-commit
  metadata repair hits transient failures.
- Revalidate MCP SSE bearer tokens during long-lived streams.
- Cap inline SSE payloads, avoid SSE counter underflow, coordinate scheduled
  reconcile with vault push locks, and time out stuck vault push locks.
- Reduce setup/login metadata leakage, prune limiters off the async runtime,
  and hide disabled-account state from Git HTTP authentication.

## [1.0.5] - 2026-05-27

### Performance

- Trim low-risk server and plugin overhead from the v1.0.4 performance review:
  narrower token/user queries, batched cleanup deletes, hot-path SQLite indexes,
  cached plugin update/history helpers, reduced duplicate utility code, and
  leaner dependency feature sets.

## [1.0.4] - 2026-05-27

### Fixed

- Address second-pass Admin and plugin review findings: safer IPv6 activity IP
  masking, CSP-compatible user filtering, accurate invite statistics,
  self-demotion session cleanup, table scrolling without clipping panels, and
  stronger plugin update recovery.
- Tighten regression coverage for plugin watchers, unspecified IPv6 server
  URLs, plugin diff semantics, invite counters, and reviewed Admin behavior.

### Performance

- Reduce small blocking filesystem checks in blob storage, avoid repeated path
  lowercase allocations in text classification, and reuse already downloaded
  non-inline text content during plugin pulls.

## [1.0.3] - 2026-05-27

### Fixed

- Harden setup CSRF, admin rollback identity handling, activity metadata,
  resource pruning, user serialization, and CLI materialize vault ID
  validation.
- Restore Admin WebUI styling and usability details after the Working Paper
  refresh, including localized templates, warning panels, table scrolling,
  focus shapes, dashboard collection behavior, and activity IP masking.
- Improve Obsidian plugin behavior and review ergonomics: clear unload sync
  timeouts, debounce blur syncs, reject unsafe unspecified HTTP server URLs,
  and enlarge split diff modals for code-review-style reading.

## [1.0.2] - 2026-05-26

### Fixed

- Polish localized Admin WebUI surfaces across the five supported languages,
  including page headers, vault/user/detail views, settings forms, and token
  management copy.
- Show Admin footer version information instead of a live clock, and remove the
  unused footer clock updater.
- Keep device token secrets one-time only: existing token rows now show stable
  fingerprints instead of raw token material.
- Replace decorative placeholder marks in Admin user/device lists with useful
  vault count and last-sync metadata.

## [1.0.1] - 2026-05-26

### Fixed

- Restore clearer Admin WebUI control affordances in the Working Paper design,
  including buttons, icon actions, form controls, segmented choices, and danger
  actions.
- Improve Admin WebUI responsive controls and icon semantics for the sidebar,
  mobile menu, close action, rollback action, and user avatars.

## [1.0.0] - 2026-05-25

**PKV Sync 1.0** is the first stable release. It freezes the documented
public `/api/*` contract in `public-docs/openapi.yaml`, establishes semantic
versioning for the 1.x line, and consolidates the storage schema into a single
v1 SQLite baseline for fresh 1.x deployments.

### Added

- Stable public REST contract, published as OpenAPI 3.0 with experimental
  surfaces explicitly marked.
- Five-language public documentation set: English, Simplified Chinese,
  Traditional Chinese, Japanese, and Korean for README, security policy, and
  all public Markdown guides.
- 1.0 upgrade notes that document the supported 0.x to 1.0 path: back up,
  export or materialize vault data, start a fresh 1.0 data directory, then
  import or push content into the new server.

### Changed

- Server, plugin manifest, package metadata, lock files, Docker/release
  documentation, and public docs are aligned on version `1.0.0`.
- SQLite migrations are squashed into `0001_initial.sql` as the v1 baseline.
  Published 1.x migrations after this baseline are append-only.
- Admin Web UI and the Obsidian plugin now expose a single theme-mode button
  that cycles through automatic, light, and dark modes.
- Admin language selection uses a compact dropdown, and plugin language
  selection remains usable for longer localized language names.
- Activity rows now label the vault column as "Vault" instead of "Detail".
- Manual light/dark mode in the Obsidian plugin now wins over the current
  Obsidian app theme.
- Admin Web UI and Obsidian plugin adopt a unified "Working Paper" visual
  design: ink-on-vellum palette with a terracotta accent, monospaced
  metadata labels, a `[PKV/SYNC]` wordmark, page-load reveal animation,
  and a discreet colophon footer.
- README rewritten across all five languages to lead with what PKV Sync
  is in plain language; reference material (full CLI surface, HTTP API
  contract, operations runbook, configuration table, versioning policy)
  is now linked out to the existing `public-docs/*` manuals.
- Plugin and admin language strings are self-contained per locale:
  `zh-Hant` no longer inherits Simplified Chinese text, and `ja`/`ko` no
  longer fall back to English on a handful of strings.

### Fixed

- Per-vault push locking no longer serializes through a global mutex.
- SSE reconnect subscribes before replay, closing the window where commits
  created during reconnect could be missed.
- Pull and upload-check hot paths avoid avoidable blocking and repeated work
  for large vaults.
- Plugin self-update writes are post-write verified and recover from leftover
  temporary or backup files on startup.
- Background settings writes in the plugin partial-merge only their own fields,
  avoiding clobbering concurrent settings edits.
- Plugin vault scans skip rehashing unchanged files when file metadata proves
  the cached hash is still current.
- Vault name uniqueness is enforced by SQLite instead of a race-prone
  list-then-check path.
- MCP vault listing reads vault heads concurrently and no longer carries
  unused public-config code.
- Queued pushes re-check vault ownership after acquiring the per-vault push
  lock, so deleting a vault cannot be followed by a stale push that recreates
  orphaned git storage.

### Security

- Security policy now defines supported versions, disclosure channels, and
  response targets for the 1.x line.
- Release documentation now explicitly states that 0.x is unsupported after
  the 1.0 release.

### Breaking

- 1.0 uses a new v1 SQLite migration baseline. Existing 0.x `metadata.db`
  files are not supported for in-place upgrade by the 1.0 binary or container.
  Follow `public-docs/upgrade-notes-v1.0.md` before starting 1.0 against
  production data.

## [0.8.4] - 2026-05-24

### Fixed

- **Plugin no longer wipes user-configured `extraExcludeGlobs` on every
  config refresh**: the server config endpoint never emits
  `extra_exclude_globs`, but the plugin previously read it as `?? []` and
  silently reset the user setting. The field has been removed from the
  plugin/server contract; the per-vault `extra_sync_globs` allowlist remains
  the live path.
- **SSE reconnect with `Last-Event-Id` no longer misclassifies attachments
  as text**: the replay path now parses the `pkvsync_pointer` JSON marker
  and emits `kind: "blob"` with `blob_hash` + real `size` instead of
  reporting every replayed change as `text_ref` with the JSON pointer
  length. Clients reconnecting after a network blip will now fetch
  attachments through the right code path again.
- **Background update check no longer clears a known available-update
  banner on a transient GitHub failure**: only `Ok(Some(_))` overwrites
  the status; rate-limit / 5xx responses leave the previous banner intact
  until the next successful tick.
- **First-run setup wizard creates the first admin atomically**: rebuilt
  on top of a new `UserRepo::create_first_admin` that uses
  `INSERT ... WHERE NOT EXISTS (SELECT 1 FROM users WHERE is_admin = 1)`,
  so two concurrent `/setup` POSTs from different IPs can no longer both
  pass the `count_admins() == 0` check and create two admin rows.
- `/api/vaults/:id/files/:path` blob response avoids an extra `Vec<u8>`
  copy for large attachments; both arms now return `Bytes` directly.
- Plugin SSE reconnect backoff resets as soon as the response is OK,
  rather than waiting for the first commit event. An open-then-immediately-
  closed stream no longer leaves the next reconnect pinned at 30 s.
- Plugin no longer double-registers poll/fallback timers with Obsidian's
  `registerInterval` while still clearing them manually, so rebuilding the
  sync engine across settings edits no longer accumulates stale auto-clear
  entries.
- Admin dashboard now translates `extra_exclude_globs` and its hint into
  Japanese and Korean instead of falling back to English.

### Added

- **Admin dashboard "Sync Status" card now reflects live state**: total
  SSE subscribers across all vaults, the most recent `sync_activity`
  timestamp, and a three-state badge (live / idle / quiet) replacing the
  previous static "All systems healthy" placeholder.
- **Admin dashboard "Version" card** showing the running server version,
  "Up to date" or `v{latest} available` derived from the background
  update check, and the relative time of the last successful update-check
  HTTP roundtrip. The card is always visible, so operators can confirm
  the update-check pipeline is alive even when no newer release exists.
- New `VaultEventBus::total_subscribers()` aggregate used by the
  dashboard, plus a regression test that asserts the legacy
  "All systems healthy" placeholder is not rendered.

## [0.8.3] - 2026-05-24

### Fixed

- Relaxed the global `Referrer-Policy` from `no-referrer` to `same-origin`
  so first-run setup and admin form POSTs are no longer blocked by
  browsers serializing `Origin: null` per the Fetch spec under
  `no-referrer`. CSRF stays strict — `Origin: null` is rejected by a
  dedicated regression test.

## [0.8.2] - 2026-05-23

### Fixed

- Kept setup wizard CSRF aligned with the configured public HTTPS origin so
  `public_host = "sync.example.com"` works behind reverse proxies that forward
  the backend scheme as HTTP.
- Clarified `public_host` docs in README and deployment hardening guides.

## [0.8.1] - 2026-05-23

### Security

- Hardened the bundled plugin manifest endpoint so asset URLs use configured
  `public_host` when available and no longer trust unverified
  `X-Forwarded-Proto` request headers.
- Added the deployment key middleware to MCP Streamable HTTP transport, while
  keeping bearer-token authentication for MCP user authorization.
- Added default security response headers for clickjacking, MIME sniffing,
  referrer, CSP, and HSTS when `public_host` is configured.
- Normalized MCP file paths before read, write, and delete operations.
- Included MCP write limiter cleanup in the periodic limiter pruning task.

### Documentation

- Updated README, OpenAPI, CLI reference, MCP how-to, user manuals, and
  deployment hardening docs to match the current MCP write, deployment-key, and
  plugin self-update behavior.

## [0.8.0] - 2026-05-23

### Added

- **First-run setup wizard**: opening a fresh PKV Sync server in a browser now
  shows a setup page where the operator chooses a username and password for the
  first administrator account. No more random passwords printed to stderr or
  container logs.
- **Server update check**: the admin dashboard shows a banner when a newer PKV
  Sync release is available on GitHub. Checks run every 24 hours by default;
  air-gapped deployments can disable them with `[update_check] enabled = false`.
- **`pkvsyncd upgrade` CLI**: downloads the latest release binary side-by-side
  as `pkvsyncd.new`, verifies SHA-256 from `SHA256SUMS`, and prints
  systemd/manual swap instructions. Docker and Kubernetes deployments are
  detected and redirected to image-pull guidance.
- **Plugin update check and self-update**: the Obsidian plugin settings panel
  shows when a newer plugin version is available, preferring the connected PKV
  Sync server's bundled plugin assets and falling back to GitHub releases.
  "Update now" downloads, verifies, and writes plugin assets, then prompts for
  an Obsidian reload.
- **Server `/api/plugin-manifest` endpoint**: lets authenticated plugin clients
  discover the server's bundled plugin version, SHA-256 hashes, and download
  URLs for `main.js`, `manifest.json`, and `styles.css`.

### Changed

- **BREAKING for first-time deployments only**: on a fresh database, the server
  no longer auto-creates an `admin` user with a random password. The first
  browser request redirects the operator through the setup wizard. Existing
  deployments with at least one admin user seal setup immediately at boot and
  are not affected.
- **Setup-required API state**: when the server has no admin users, `/api/*`
  returns `503 Service Unavailable` with `error.code = "setup_required"`. The
  plugin surfaces this as a setup-required notice.

## [0.7.0] - 2026-05-23

### Added

- Vault-level rollback: `POST /api/vaults/:id/restore` moves a vault HEAD to
  a selected historical commit after typed vault-name confirmation. The
  Obsidian history modal exposes "Rollback to here", the Admin WebUI exposes
  rollback controls from vault history, and `vault_rollback` activity records
  `from_commit` / `to_commit` for audit.
- Rollback SSE events: subscribed clients receive `kind: "rollback"` with the
  old and new commit ids, then perform a full pull so every device aligns with
  the restored vault state.
- Obsidian Sync migration: the plugin command "Import current vault to PKV Sync"
  scans the current vault, skips Sync internals, workspace/cache/trash/git and
  PKV plugin files, creates a new PKV Sync vault, uploads text and binary files
  in batches, and records the migration as the first PKV Sync commit.
- Public migration guide for moving current files from Obsidian Sync to PKV
  Sync in English, Simplified Chinese, Traditional Chinese, Japanese, and
  Korean.
- Prometheus counter `pkv_vault_rollback_total`.

### Changed

- `VaultEvent` JSON now carries a `kind` field. Normal commit events continue
  to include the existing `changes` array, while rollback events carry
  `from_commit` and `to_commit`.
- Admin documentation no longer describes the Admin WebUI history surface as
  read-only for rollback, and now points operators to the available rollback
  controls.

## [0.6.0] - 2026-05-23

### Added

- Text three-way auto-merge for stale text pushes. When two devices edit the
  same text file from different baselines, PKV Sync now tries a git-style line
  merge before falling back to conflict files. Clean disjoint edits are merged
  automatically, while overlapping edits create `.conflict-*` files containing
  `<<<<<<< local`, `=======`, and `>>>>>>> remote` markers for manual
  resolution.
- Admin runtime setting `enable_auto_merge` defaults on and can disable the
  auto-merge path when operators need the previous conflict behavior.
- Obsidian conflict resolution now recognizes conflict files with merge markers,
  previews marker blocks, opens them in the editor, and lets users mark them
  resolved after the markers are removed.
- MCP write tools `write_file` and `delete_file` create, update, or delete vault
  files through the normal sync pipeline with optimistic `parent_commit` checks.
  Stale MCP writes return the current head instead of overwriting newer data.
- MCP writes are rate-limited per token and vault at 60 writes per minute and
  successful writes are recorded as `mcp_write` or `mcp_delete` activity.
- Traditional Chinese, Japanese, and Korean coverage for the Obsidian plugin,
  admin WebUI, and public documentation. Japanese and Korean are marked as
  review-needed community translations.
- CI now runs `scripts/i18n_check.py` to keep plugin language key coverage
  aligned, and the Grafana dashboard includes an auto-merge success-rate panel.

### Changed

- MCP documentation now describes write tools, optimistic concurrency, write
  rate limiting, audit logging, and the fact that AI-driven writes enter git
  history.
- Public documentation language switchers now include English, Simplified
  Chinese, Traditional Chinese, Japanese, and Korean variants.

## [0.5.3] - 2026-05-22

### Fixed

- Active device tokens no longer silently expire on the 90-day mark. Every
  authenticated request now extends token expiry by 90 days from the request
  time, while idle devices still expire after 90 days without use.
- SSE subscriber limits are now enforced per user, with a default cap of 16
  concurrent subscribers per user and a global ceiling of 1024. One
  authenticated client can no longer exhaust the shared SSE quota for every
  other user.

## [0.5.2] - 2026-05-22

### Changed

- Reduced redundant CI work by removing duplicate plugin verification steps
  already covered by the package and coverage jobs.
- Reused compiled plugin path matchers across scan and pull filtering passes,
  and reduced SHA-256 hex conversion allocation overhead.
- Shared plugin API path segment encoding between history and sync clients.
- Batched server blob reference and upload availability checks during sync
  pushes.
- Removed an unused vault settings repository lookup.

## [0.5.1] - 2026-05-21

### Fixed

- Added abuse protection for authenticated sync API routes and MCP Streamable
  HTTP requests.
- Scoped idempotency cache keys by user, vault, and route, with a migration for
  existing deployments.
- Cleaned up vault event subscriptions when a vault is deleted.
- Hardened plugin glob character-class handling so regex escapes are treated as
  literals.
- Rejected insecure non-loopback SSE URLs before sending credentials.
- Clamped server-provided sync debounce values and filtered server-provided text
  extensions against the plugin allowlist.
- Flushed pending plugin sync work during Obsidian unload.
- Required admin authentication for metrics, capped SSE subscribers and replay
  history, hid internal error details, preserved registration rate-limit budget,
  validated Git HTTP vault ids, and normalized admin session and language
  redirect behavior.

### Security

- Documented that the Obsidian plugin stores the active device token and
  deployment key in vault-local plugin data and that the file should be treated
  as sensitive.
- Added regression coverage confirming push JSON bodies remain protected by
  Axum's default body limit.

## [0.5.0] - 2026-05-21

### Added

- Prometheus `/metrics` endpoint gated by the `enable_metrics` runtime setting
  and deployment key, with counters and gauges for HTTP traffic, sync activity,
  SSE subscribers, token/vault totals, repository size, and blob GC activity.
- Grafana dashboard template for the PKV Sync metrics surface under
  `deploy/grafana/`.
- `pkvsyncd backup`, `pkvsyncd restore`, and `pkvsyncd verify` operator CLI
  subcommands for consistent snapshots, restore validation, blob hash checks,
  orphan blob reporting, and git repository verification.
- Rust and plugin coverage reporting in CI, with an Ubuntu-only tarpaulin run,
  Vitest V8 coverage, public coverage baselines, uploaded artifacts, and a
  no-regression gate that allows at most a 5 percentage point drop.

### Changed

- Documentation now recommends the operational backup, restore, and verify CLI
  flow instead of manual data-directory copies.
- Obsidian SSE handling now keeps Last-Event-ID state inside the fetch event
  client and reconnects automatically with exponential backoff.

## [0.4.1] - 2026-05-20

### Fixed

- Obsidian SSE subscriptions now reconnect automatically with exponential
  backoff after network changes, restoring realtime sync without restarting
  Obsidian or saving settings.
- Obsidian plugin settings layout is less cramped and uses more consistent
  control sizing.
- Server blob pushes now require blobs to be uploaded or already referenced by
  the same vault, preventing cross-vault blob grafting by known hash.
- Git smart HTTP `upload-pack` responses are streamed instead of buffered in
  memory.
- MCP search results are capped server-side, and MCP blob reads now require
  the blob to be referenced by the requested vault.
- Materialize rejects malformed blob pointer hashes instead of trusting pointer
  data.

## [0.4.0] - 2026-05-19

### Added

- `pkvsyncd mcp` starts a read-only MCP server for vault access through stdio
  or stateless Streamable HTTP. Tools include `list_vaults`, `list_files`,
  `read_file`, `read_file_at_commit`, and `search`.
- MCP Streamable HTTP supports `vault_changed` SSE notifications and
  `Last-Event-ID` replay using `<vault-id>:<commit-sha>` event ids.
- Per-vault sync settings store `extra_sync_globs` for selective hidden-path
  sync, including a starter `.obsidian` allowlist for new vaults.
- `GET /api/vaults/:id/settings` and `PUT /api/vaults/:id/settings` expose
  per-vault settings to authenticated clients.
- Admin WebUI vault settings page for editing per-vault sync allowlists and
  applying the recommended starter allowlist.
- Obsidian plugin settings editor for `.obsidian` sync rules, with i18n and
  sync-engine integration.
- Regression coverage for `.obsidian` and hidden-path sync edge cases,
  including nested paths, deletes, rename-style delete/upsert batches, and
  hard excludes.

### Changed

- Sync path filtering now applies hard excludes first, per-vault hidden-path
  allowlists second, and global exclude globs last.
- `/api/vaults/:id/events` now emits commit ids and replays missed commit
  events when clients reconnect with `Last-Event-ID`.
- New vaults automatically receive the starter `.obsidian` sync allowlist;
  existing vaults remain empty until users or admins opt in.

## [0.3.8] - 2026-05-19

### Added

- Conflict resolution is now available from the Obsidian file context menu
  for both the original file and its generated `.conflict-*` file.

### Changed

- Obsidian plugin settings now follow Obsidian light and dark themes with
  palettes aligned to the Admin WebUI.

## [0.3.7] - 2026-05-19

### Fixed

- SSE subscriptions from Obsidian now send `X-PKVSync-Plugin` in addition to
  `User-Agent`, and the server accepts that header only on `/events`. This
  fixes browsers that do not reliably apply a custom `User-Agent` on `fetch()`
  while keeping the normal plugin identity check for the SSE request.
- SSE CORS allow-headers now include `x-pkvsync-plugin` from the shared SSE
  header list, so preflight and middleware rejection responses stay in sync.
- Conflict resolution now recognizes generated conflict filenames that preserve
  the original extension, so the resolver can show both the local file and the
  conflict file instead of only one side.

## [0.3.6] - 2026-05-18

### Fixed

- SSE CORS error "No 'Access-Control-Allow-Origin' header is present" when
  the UA-filter or deployment-key middleware rejects the request. Both
  middlewares sit outside the SSE route's CorsLayer, so their 404 rejection
  responses carried no CORS headers, causing the browser to block the
  response. Now, when a request to `/events` carries a cross-origin `Origin`
  header, rejection responses include `Access-Control-Allow-Origin: *` and
  the full set of `Access-Control-Allow-Headers`, so the browser surfaces
  the actual error status instead of a generic CORS failure.

## [0.3.5] - 2026-05-18

### Fixed

- SSE CORS preflight now includes `User-Agent` in `Access-Control-Allow-Headers`.
  When the plugin sets a custom `User-Agent` header on the SSE `fetch()` call
  (added in v0.3.4), browsers include it in the CORS preflight
  `Access-Control-Request-Headers`. The server's SSE CorsLayer must whitelist
  it, or the preflight is rejected and the plugin falls back to polling.

## [0.3.4] - 2026-05-18

### Fixed

- SSE subscription `fetch()` now sends `User-Agent: PKVSync-Plugin/X.Y.Z`
  instead of the browser's default UA. Electron 32 (Chromium 128+) supports
  setting `User-Agent` in `fetch()`, so the server-side UA filter validates
  SSE requests normally — no special `/events` bypass needed.
- Removed the `/events` path bypass in the UA filter middleware. All
  non-OPTIONS requests must carry a valid plugin User-Agent; the previous
  workaround that let any UA through on SSE GET requests is no longer
  necessary.

## [0.3.3] - 2026-05-18

### Fixed

- SSE subscription failed with `TypeError: Failed to fetch` in the Obsidian
  plugin behind any reverse proxy. The browser sends a CORS preflight
  OPTIONS for the cross-origin `fetch` with `Authorization` and
  `X-PKVSync-Deployment-Key` headers; the previous build rejected the
  preflight at the deployment-key and UA-filter middlewares (both returned
  404 for any request lacking those values), and there was no CORS layer
  on the SSE route to answer the preflight. Result: every plugin client
  silently fell back to polling, and end-to-end sync took 20-30 s even on
  LAN instead of the sub-second target.
- Both middlewares now pass OPTIONS requests through; the SSE route gets
  a `tower_http::cors::CorsLayer` that whitelists `Authorization`,
  `Accept`, `Cache-Control`, `Last-Event-ID`, and the deployment-key
  custom header. Authentication for the actual request is unchanged —
  bearer token and deployment key are still required for the GET that
  follows the preflight.

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
