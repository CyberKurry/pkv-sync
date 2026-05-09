# PKV Sync Admin Manual

English | [简体中文](./admin-manual.zh-CN.md)

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
  actions
- Invite creation with optional expiration, active invite listing, and deletion
  for unused invites
- Runtime settings grouped as General, Security, Sync & Storage, and Network
- Activity log with real user and action filters for push and pull rows
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

Device bearer tokens are valid for 90 days. Users can revoke their own tokens,
and administrators can revoke tokens for any user.

Operational notes:

- Token plaintext is shown only once at creation.
- Only SHA-256 token hashes are stored in the database.
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

## Activity

The activity log records sync operations such as push and pull, including:

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

- Keep `config.toml`, `metadata.db`, `vaults/`, and `blobs/` in the same backup
  set.
- Run blob garbage collection after large attachment deletions.
- Watch logs and activity for repeated `401`, `403`, `404`, `409`, and `429`
  responses.
- Keep the server binary, plugin package, Docker image, reverse proxy, and host
  OS patched.
- Verify CI before tagging a release.
- Check each release contains Linux amd64, Linux arm64, Windows x64, plugin zip,
  checksums, and GHCR Docker image tags.
