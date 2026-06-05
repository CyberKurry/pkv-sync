# PKV Sync Admin Manual

English | [简体中文](./admin-manual.zh-CN.md) | [繁體中文](./admin-manual.zh-Hant.md) | [日本語](./admin-manual.ja.md) | [한국어](./admin-manual.ko.md)

Document version: v1.0.13.

This manual covers day-to-day administration for a self-hosted PKV Sync server.
For network and host hardening, read the deployment hardening guide as well.

## First Run

1. Generate a deployment key:

   ```bash
   pkvsyncd genkey
   ```

2. Create `/etc/pkv-sync/config.toml` from `config.example.toml`.
3. Initialize the v1 database baseline for a fresh 1.x data directory:

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml migrate up
   ```

4. Start the server:

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml serve
   ```

5. On a fresh database, open `/setup` in a browser and create the first
   administrator account. PKV Sync no longer prints a random admin password to
   stderr or container logs.
6. After setup completes, use `/admin/login` for normal administrator sign-in.

PKV Sync 1.0 uses a single v1 SQLite baseline. Databases created by 0.x
releases are not supported for in-place upgrade to 1.0.0; follow
[`upgrade-notes-v1.0.md`](./upgrade-notes-v1.0.md). After this v1 baseline,
published 1.x migrations are append-only.

## Admin Web Panel

Open:

```text
https://sync.example.com/admin/login
```

The web panel includes:

- Dashboard with system, storage, vault, user, and recent activity indicators
  plus an update banner when a newer PKV Sync release is available
- User list with search and status filters
- User detail pages for password reset, active/admin controls, and token review
- Global device token page for listing, creating, and revoking tokens
- Vault cards with owner, file count, size, last sync, reconcile, and delete
  actions, plus per-vault sync settings
- Read-only vault file browser with file previews, per-file history timelines,
  and unified diff rendering
- Invite creation with optional expiration, active invite listing, and deletion
  for unused invites
- Runtime settings grouped as General, Security, Sync & Storage, and Network
  (including update checks)
- Activity log with real user and action filters for sync, vault lifecycle,
  and read-only browsing rows
- Blob garbage collection trigger
- English, Simplified Chinese, Traditional Chinese, Japanese, and Korean language switch

Timestamps, durations, byte sizes, uptime, and activity data are rendered in
human-readable form. The default timezone is `Asia/Shanghai` and can be changed
from settings.

## Update Notifications

PKV Sync checks GitHub releases once every 24 hours by default. When a newer
server release exists, the dashboard shows a banner with the current version,
latest version, release notes link, and a short excerpt.

`[update_check].enabled` and `[update_check].interval_seconds` in
`config.toml` seed fresh databases on first boot. After that, the Admin WebUI
Settings page is authoritative: toggle update checks or change the interval
from **Network**, and the background task re-reads those runtime values on its
next cycle. If checks are disabled, re-enabling takes effect within about 60
seconds. `[update_check].repo` remains a static `config.toml` value for
air-gapped mirror deployments.

```toml
[update_check]
enabled = false
interval_seconds = 86400
repo = "cyberkurry/pkv-sync"
```

The check is informational only. PKV Sync never replaces the running server
binary or container image automatically.

## User Management

- Create users from **Users** or with the CLI.
- Usernames must be 3-32 ASCII letters, digits, `_`, `-`, or `.`.
- Admin-created, admin-reset, public registration, and user self-change passwords must be at least 12 characters and
  include uppercase, lowercase, and a digit.
- Use search and status filters on the Users page to narrow the table.
- Open a user detail page to reset passwords, enable or disable the account,
  promote or demote admin access, and inspect that user's device tokens.
- Disable users instead of deleting when you may need audit history.
- The Admin WebUI asks for confirmation before disabling users or demoting
  admins. It blocks disabling your own admin session and demoting the last
  administrator, then shows localized feedback on the user detail page.
- Do not disable every remaining administrator account.

Resetting a password from the Admin WebUI revokes that user's existing device
tokens. The user must log in again.

CLI fallback:

```bash
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user add alice --admin
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
```

## Device Tokens

Device bearer tokens renew on authenticated use, expire after 90 idle days, and
have a 365-day absolute lifetime. Users can revoke their own tokens, and
administrators can revoke tokens for any user.

Operational notes:

- Token plaintext is shown only once at creation.
- Only SHA-256 token hashes are stored in the database.
- Admin token list endpoints and tables show public token metadata only; they
  do not reveal plaintext tokens or internal expiry/revocation fields.
- Every authenticated request extends the token expiry by 90 days from that
  request time, capped at 365 days from token creation.
- Logging in again from the same stable plugin device ID replaces the previous
  active token for that device.
- Revoked tokens referenced by activity rows can be cleaned while preserving
  activity history.

## Vaults

Deleting a vault from the Admin WebUI requires an extra confirmation dialog.
Treat deletion as destructive even though unreferenced blob files may remain
until garbage collection.

Deleting a vault removes:

- the vault database row
- related metadata rows that cascade from it
- the backing bare Git repository under `data_dir/vaults/<vault-id>`
- the in-memory per-vault push lock

Blob files are content-addressed and may remain until garbage collection proves
they are unreferenced beyond the grace period.

Use vault metadata reconciliation if file counts, sizes, or blob references
look wrong after an interrupted operation. Reconciliation reads blob pointer hashes
from the tree entries and batches blob-reference repair, so it no longer has to
re-open every pointer file individually.

### Per-Vault Sync Settings

From **Vaults**, open **Settings** on a vault card to edit the per-vault
`extra_sync_globs` allowlist. This controls which hidden paths, including
selected `.obsidian` configuration files, are allowed to sync.

New vaults receive the recommended starter allowlist automatically. Existing
vaults stay empty until an admin or the vault owner applies the starter list.
The **Apply starter allowlist** action writes the exact recommended list for
themes, CSS snippets, hotkeys, app preferences, appearance preferences, and
enabled plugin lists.

### Read-Only File History

From **Vaults**, open **Browse files** on a vault card. The browser lists the
current HEAD files with size and text/binary kind. Opening a file shows a
read-only preview when the file is text, plus links to **History** and **Diff
with previous**.

The history page lists commits for that file and links to the file at each
commit and the corresponding diff. The diff page renders unified diff lines with
add/delete/hunk coloring. Binary files show metadata and do not render binary
diff content. Paths rejected by the active sync filter are hidden from file
preview, commit-list, history, and diff surfaces.

Browsing files, history, and diffs records `view_commit`, `view_history`, and
`view_diff` activity rows. Vault rollback controls are available from Admin
history; use them only after confirming the target commit, because rollback
creates a new vault state from that selected history point.

## Invites and Registration

Configure registration from **Settings**:

- `disabled`: only admins create accounts
- `invite_only`: users register with an invite code
- `open`: anyone with the deployment URL can register

Invite creation accepts an optional future expiration time. The Admin WebUI uses
human date-time input and stores Unix seconds internally. Used invites cannot be
deleted from the admin API; keep them for audit history.

Use `open` only for short windows or public deployments with additional
monitoring and rate limits.

## Runtime Settings

The Settings page edits values stored in SQLite. Changes take effect immediately
for new requests; the in-memory cache is refreshed on save.

**General** — server name, default timezone.

**Security** — registration mode (`disabled` / `invite_only` / `open`), login
failure threshold, failure window, and lock duration. The login rate limiter
counts both failed attempts and in-flight password verifications, so a burst
of concurrent guesses cannot bypass the threshold.

Authenticated sync API routes are fixed-window rate limited at 600 requests per
60 seconds per route, method, client IP, and bearer token. Limited requests
return `429` with `error.code = "rate_limited"`.
Failed bearer-token authentication attempts are also rate limited per client IP
at 120 attempts per 60 seconds, so rotating fake tokens cannot bypass the
failure budget.

**Sync & Storage**
- Max file size (default `100 MiB`). Blob upload request bodies are always
  clamped to the hard storage cap (`512 MiB` in production), even if this
  runtime setting is raised higher.
- Supported text extensions — files outside this list are treated as binary
  blobs. The list is shown read-only in the Admin WebUI; edit it via the
  `text_extensions` runtime config row (or by editing the SQLite `runtime_config`
  table directly) if you need to change it.
- Extra exclude globs — admin-tunable patterns that augment the built-in
  `.obsidian/`, `.trash/`, `.conflict-*`, `.git/` exclusion list.
- History UI and diff endpoint toggles.
- **Auto-merge text** (`enable_auto_merge`, default on): when enabled, the
  server attempts a three-way line merge before writing a conflict file.
  Disjoint edits merge cleanly; overlapping edits still produce a conflict
  file with merge markers.
- **Push debounce** (`push_debounce_ms`, default `250`): how long the plugin
  waits after a local edit settles before pushing. Lower values reduce
  end-to-end latency; higher values batch more keystrokes per push.
- **Inline SSE content cap** (`inline_content_max_bytes`, default `8192`,
  max `65536`): text changes up to this size are shipped inside the SSE
  event so receiving plugins can apply them without a separate pull.
  Larger files always fall back to pull.
- **SSE heartbeat** (`sse_heartbeat_seconds`, default `30`): keep-alive
  ticks for the event stream so idle SSE connections survive reverse
  proxies. Concurrent SSE subscriptions are capped per user at 16 by default,
  with a global ceiling of 1024. Open event streams periodically revalidate the
  bearer token and close after token revocation or account disablement.
- **Git smart HTTP** (`enable_git_smart_http`, default off): when on,
  authorised devices can `git clone https://_:<token>@host/git/<vault-id>`.
  The server also requires the `git` binary in `PATH`; the public
  `/api/config` capability reflects both flags so clients only advertise
  the feature when it actually works.
- **Prometheus metrics** (`enable_metrics`, default off): when enabled,
  `/metrics` returns Prometheus text exposition. The route still requires the
  deployment key middleware, plugin User-Agent guard, and an admin bearer
  token.

**Network and update checks** — `public_host`, bind address, trusted proxies,
and `[update_check].repo` are read from `config.toml` at startup. Update-check
enabled/disabled state and interval are runtime settings stored in SQLite; the
allowed interval range is 60 seconds to 30 days.

## Activity

The activity log records sync, vault lifecycle, and read-only browsing
operations. Examples include `push`, `pull`, `create_vault`, `delete_vault`,
`view_commit`, `view_history`, and `view_diff`. Vault creation and deletion
from the Admin WebUI, Obsidian plugin, or public API are recorded with
`create_vault` and `delete_vault` rows.

Activity rows include:

- user
- vault
- action
- device name
- file count
- byte size
- client IP
- User-Agent
- details
- timestamp

Use the activity filters to inspect a specific user or operation type.

## Sharing Server URLs

Share the URL printed by the server or Admin WebUI:

```text
https://sync.example.com/k_xxx/
```

Treat it as sensitive. It is not a user password, but it carries the deployment
key used as the first pre-authentication gate for plugin API traffic.

## Upgrading PKV Sync

For binary installs, use `pkvsyncd upgrade --dry-run` to preview the latest
release, target asset, and side-by-side path. Use `pkvsyncd upgrade --yes` to
download the verified release binary next to the current executable as
`pkvsyncd.new` (`pkvsyncd.new.exe` on Windows). The command verifies SHA-256
from `SHA256SUMS` and prints the systemd/manual swap steps. It does not hot
replace the running process.

Use `pkvsyncd upgrade --version 1.0.13` to target a specific release. If the
command cannot find a matching asset or checksum, follow the manual GitHub
release download path and verify `SHA256SUMS` yourself.

For 0.x deployments, do not point the 1.0 binary or image at an existing
`metadata.db`. Back up, materialize or export vault contents, start a fresh
1.0 data directory, and import or push the vault contents into the new server.
See [`upgrade-notes-v1.0.md`](./upgrade-notes-v1.0.md).

Docker and Kubernetes deployments should upgrade by pulling or changing the
container image tag, then restarting the service or rollout. The upgrade CLI
detects container environments and prints image-based guidance instead of
writing a side-by-side binary.

## Maintenance Checklist

- Use `pkvsyncd backup --output <dir> [--data-dir <dir>] [--gzip]` for
  operational snapshots. The output directory must be absent or empty; the
  command snapshots SQLite with `VACUUM INTO`, copies `vaults/` and `blobs/`,
  and writes `MANIFEST.json` with the pkvsyncd version plus component hashes,
  sizes, and counts. Backups omit `config.toml` by default; add
  `--include-config` only when you intend to store and protect deployment keys
  and other local secrets.
- Use `pkvsyncd restore --input <backup-dir> --data-dir <dir>` to restore into
  an absent or empty data directory. Add `--force` only when the target may be
  cleared first; restore checks manifest hashes before copying and runs verify
  afterward.
- Use `pkvsyncd verify [--data-dir <dir>]` after maintenance or host storage
  incidents. It checks referenced blob files, reports orphan blobs, validates
  vault git repos with `git2`, and exits non-zero for missing, corrupt, or git
  errors. `--no-fail` keeps the report but forces a success exit code.
- Use `pkvsyncd materialize <vault-id> -o <dir>` to export a vault's HEAD
  as a plain file tree (text files as-is, binary blobs resolved from the
  blob store). Useful for offline export, ad-hoc audit, or cold migration.
  Pair with `--at <commit-sha>` to materialize a historical commit.
- Set `[mcp].embed_in_serve = true` to expose the read/write MCP Streamable
  HTTP endpoint at `/mcp` on the main `pkvsyncd serve` port, or run
  `pkvsyncd mcp --transport http --bind 127.0.0.1:6711` as a standalone MCP
  process. Use `pkvsyncd mcp --vault <id>` for a stdio-only single-vault
  session.
- Run blob garbage collection after large attachment deletions.
- Check the dashboard update banner or GitHub releases before maintenance.
- Watch logs and activity for repeated `401`, `403`, `404`, `409`, and `429`
  responses.
- Keep the server binary, plugin package, Docker image, reverse proxy, and host
  OS patched.
- Verify CI before tagging a release.
- Check each release contains Linux amd64, Linux arm64, Windows x64, plugin zip,
  checksums, and GHCR Docker image tags.
