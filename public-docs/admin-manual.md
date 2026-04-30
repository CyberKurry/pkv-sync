# PKV Sync Admin Manual

English | [简体中文](./admin-manual.zh-CN.md)

## First Run

1. Generate a deployment key:

   ```bash
   pkvsyncd genkey
   ```

2. Create `/etc/pkv-sync/config.toml` from `config.example.toml`.
3. Apply migrations:

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml migrate up
   ```

4. Start the server:

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml serve
   ```

5. Save the first-run admin password printed to stderr.

## Admin Web Panel

Open:

```text
https://sync.example.com/admin/login
```

Use the first-run admin credentials, then change the password.

The web panel includes dashboard, users, invites, runtime settings, activity,
and blob garbage collection pages.

## User Management

- Create users from the **Users** page.
- Disable users instead of deleting when you may need audit history.
- Resetting a password revokes existing device tokens.
- Do not demote or disable your last active admin account.

CLI fallback:

```bash
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
```

## Registration Modes

Configure registration from the **Settings** page:

- `disabled`: admin creates accounts manually
- `invite_only`: users register with an invite code
- `open`: anyone with the deployment URL can register

Use `open` only for short windows or public deployments with additional
monitoring.

## Sharing Server URL

Share the URL printed by the server:

```text
https://sync.example.com/k_xxx/
```

Treat this as sensitive. It is not a password, but it is the first pre-auth
gate and should not be posted publicly for a private server.

## Maintenance

- Monitor dashboard CPU, memory, users, vaults, and disk indicators.
- Run blob garbage collection after large deletions.
- Keep encrypted backups of metadata, vault Git repos, blobs, and config.
- Watch logs for repeated 401, 403, 404, 409, and 429 responses.
- Keep the server binary, reverse proxy, and host OS patched.
