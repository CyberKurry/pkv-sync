# PKV Sync Admin Manual

English | [简体中文](./admin-manual.zh-CN.md) | [繁體中文](./admin-manual.zh-Hant.md) | [日本語](./admin-manual.ja.md) | [한국어](./admin-manual.ko.md)

This manual covers day-to-day administration for a self-hosted PKV Sync server.
For network and host hardening, read the deployment hardening guide as well.

## First Run

1. Generate a deployment key:

   ```bash
   pkvsyncd genkey
   ```

2. Create `/etc/pkv-sync/config.toml` from `config.example.toml`.
3. Apply database migrations:

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml migrate up
   ```

4. Start the server:

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml serve
   ```

5. Save the first-run admin password printed to stderr or container logs.
6. Open `/admin/login`, sign in as `admin`, and change the password.

Migrations are intentionally append-only after release. Do not squash or edit
already published migration files for an existing deployment.

## Admin Web Panel

Open:

```text
https://sync.example.com/admin/login
```

The web panel includes:

- Dashboard with system, storage, vault, user, and recent activity indicators
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
- Activity log with real user and action filters for sync, vault lifecycle,
  and read-only browsing rows
- Blob garbage collection trigger
- English and Simplified Chinese language switch

Timestamps, durations, byte sizes, uptime, and activity data are rendered in
human-readable form. The default timezone is `Asia/Shanghai` and can be changed
from settings.

## User Management

- Create users from **Users** or with the CLI.
- Usernames must be 3-32 ASCII letters, digits, `_`, `-`, or `.`.
- Use search and status filters on the Users page to narrow the table.
- Open a user detail page to reset passwords, enable or disable the account,
  promote or demote admin access, and inspect that user's device tokens.
- Disable users instead of deleting when you may need audit history.
- Do not demote or disable the last active admin account.

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

Device bearer tokens renew on authenticated use and expire after 90 idle days.
Users can revoke their own tokens, and administrators can revoke tokens for any
user.

Operational notes:

- Token plaintext is shown only once at creation.
- Only SHA-256 token hashes are stored in the database.
- Every authenticated request extends the token expiry by 90 days from that
  request time without shortening a later expiry.
- Logging in again from the same stable plugin device ID replaces the previous
  active token for that device.
- Revoked tokens referenced by activity rows can be cleaned while preserving
  activity history.

## Vaults

Deleting a vault removes:

- the vault database row
- related metadata rows that cascade from it
- the backing bare Git repository under `data_dir/vaults/<vault-id>`
- the in-memory per-vault push lock

Blob files are content-addressed and may remain until garbage collection proves
they are unreferenced beyond the grace period.

Use vault metadata reconciliation if file counts, sizes, or blob references
look wrong after an interrupted operation.

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
diff content.

The Admin WebUI intentionally has no restore, revert, rollback, or write-back
controls. Browsing files, history, and diffs records `view_commit`,
`view_history`, and `view_diff` activity rows.

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

**Sync & Storage**
- Max file size (default `100 MiB`).
- Supported text extensions — files outside this list are treated as binary
  blobs.
- Extra exclude globs — admin-tunable patterns that augment the built-in
  `.obsidian/`, `.trash/`, `.conflict-*`, `.git/` exclusion list.
- History UI and diff endpoint toggles.
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
  with a global ceiling of 1024.
- **Git smart HTTP** (`enable_git_smart_http`, default off): when on,
  authorised devices can `git clone https://_:<token>@host/git/<vault-id>`.
  The server also requires the `git` binary in `PATH`; the public
  `/api/config` capability reflects both flags so clients only advertise
  the feature when it actually works.
- **Prometheus metrics** (`enable_metrics`, default off): when enabled,
  `/metrics` returns Prometheus text exposition. The route still requires the
  deployment key middleware, plugin User-Agent guard, and an admin bearer
  token.

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

## Maintenance Checklist

- Use `pkvsyncd backup --output <dir> [--data-dir <dir>] [--gzip]` for
  operational snapshots. The output directory must be absent or empty; the
  command snapshots SQLite with `VACUUM INTO`, copies `vaults/`, `blobs/`, and
  `config.toml` when present, and writes `MANIFEST.json` with the pkvsyncd
  version plus component hashes, sizes, and counts.
- Use `pkvsyncd restore --input <backup-dir> --data-dir <dir>` to restore into
  an absent or empty data directory. Add `--force` only when the target may be
  cleared first; restore checks manifest hashes before copying and runs verify
  afterward.
- Use `pkvsyncd verify [--data-dir <dir>]` after maintenance or host storage
  incidents. It checks referenced blob files, reports orphan blobs, validates
  vault git repos with `git2`, and exits non-zero for missing, corrupt, or git
  errors. `--no-fail` keeps the report but forces a success exit code.
- Run blob garbage collection after large attachment deletions.
- Watch logs and activity for repeated `401`, `403`, `404`, `409`, and `429`
  responses.
- Keep the server binary, plugin package, Docker image, reverse proxy, and host
  OS patched.
- Verify CI before tagging a release.
- Check each release contains Linux amd64, Linux arm64, Windows x64, plugin zip,
  checksums, and GHCR Docker image tags.
