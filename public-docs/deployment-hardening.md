# PKV Sync Deployment Hardening Guide

English | [简体中文](./deployment-hardening.zh-CN.md) | [繁體中文](./deployment-hardening.zh-Hant.md) | [日本語](./deployment-hardening.ja.md) | [한국어](./deployment-hardening.ko.md)

This guide assumes a small self-hosted deployment for yourself, family, a team,
or a trusted group of friends. PKV Sync is operationally simple, but it stores
readable vault contents on the server, so host and backup hygiene matter.

## Threat Model

PKV Sync does not provide end-to-end encryption. Protecting vault contents
depends on layered controls:

1. HTTPS transport encryption
2. Deployment key pre-authentication
3. Username/password login and bearer device tokens that renew on use
4. Per-user vault authorization checks
5. Admin session and CSRF protections
6. OS or provider disk encryption
7. Minimal exposed services
8. Encrypted, tested backups

Treat the server administrator and the server filesystem as trusted with the
plaintext vault contents.

## Recommended Topology

```text
Internet -> HTTPS reverse proxy -> 127.0.0.1:6710 pkvsyncd
```

Do not expose `pkvsyncd` directly to the internet unless you have an explicit
network control layer in front of it.

## Installation Inputs

Prepare:

- a domain such as `sync.example.com`
- a deployment key from `pkvsyncd genkey`
- `/etc/pkv-sync/config.toml`
- a persistent data directory, commonly `/var/lib/pkv-sync`
- a reverse proxy with valid TLS certificates

The server share URL has this form:

```text
https://sync.example.com/k_xxx/
```

Keep it private. The deployment key is a pre-authentication gate for API
traffic, not a replacement for user passwords.

## System User

```bash
sudo useradd --system --home /var/lib/pkv-sync --shell /usr/sbin/nologin pkv-sync
sudo mkdir -p /var/lib/pkv-sync /etc/pkv-sync
sudo chown -R pkv-sync:pkv-sync /var/lib/pkv-sync
sudo chmod 750 /var/lib/pkv-sync
```

Store `config.toml` in `/etc/pkv-sync/config.toml` and keep it readable only by
the service user and administrators.

## Firewall

Expose only SSH and HTTPS on a typical host:

```bash
sudo ufw allow OpenSSH
sudo ufw allow 443/tcp
sudo ufw enable
```

If Caddy or another ACME HTTP-01 client manages certificates, also expose port
`80` for validation and redirect traffic:

```bash
sudo ufw allow 80/tcp
```

Bind `pkvsyncd` to localhost when it runs on the host:

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

For Docker Compose, bind the app to all container interfaces and publish the
host port only to localhost when you need host debugging:

```toml
[server]
bind_addr = "0.0.0.0:6710"
```

```yaml
ports:
  - "127.0.0.1:6710:6710"
```

## Docker Compose With Caddy

Use this path when you want Caddy to request and renew HTTPS certificates.

1. Point DNS at the server:

   ```text
   sync.example.com A    <server IPv4>
   sync.example.com AAAA <server IPv6, optional>
   ```

2. Create `config.toml` next to `docker-compose.yml`:

   ```toml
   [server]
   bind_addr = "0.0.0.0:6710"
   deployment_key = "k_replace_me"
   public_host = "sync.example.com"

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]

   [logging]
   level = "info"
   format = "json"
   ```

3. Replace `sync.example.com` in `deploy/caddy/Caddyfile`.
4. Start the stack:

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

5. Save the first-run admin password from the logs, then open:

   ```text
   https://sync.example.com/admin/login
   ```

Back up `./data`, `config.toml`, and Caddy's named volumes.

Upgrade with:

```bash
docker compose pull
docker compose up -d
docker compose logs -f pkv-sync
```

## public_host (required for admin POST)

Set `[server].public_host` to the externally-visible hostname (and port, if
non-standard) that operators use to reach the admin panel — for example
`sync.example.com` or `pkv.local:8443`. The admin CSRF check derives its
expected origin from this value.

If `public_host` is empty, every admin POST is rejected with `403 csrf
validation failed` and a `tracing::warn` log line. This is intentional
fail-closed behaviour: the alternative — falling back to the request's own
`Host` header — couples authentication to attacker-influenceable headers and
breaks when proxies forward an inconsistent host.

`public_host` also drives:

- Production-style admin cookies (`Secure`, `SameSite=Strict`) when set.
- `https://` share URL generation for the in-admin "share server URL" link.
- The expected proto used when `X-Forwarded-Proto` is missing.

For SSE, the same setting helps reverse proxies recognise that the route is a
keep-alive event stream rather than a normal short-lived request.

## Reverse Proxy Notes

### Caddy

```caddyfile
sync.example.com {
  reverse_proxy 127.0.0.1:6710
}
```

### Nginx

The repository includes `deploy/nginx/pkv-sync.conf`. It redirects HTTP to
HTTPS, sets `client_max_body_size 110m`, and forwards the headers PKV Sync uses
for host and client IP handling.

Minimum shape:

```nginx
server {
  listen 80;
  server_name sync.example.com;
  return 301 https://$host$request_uri;
}

server {
  listen 443 ssl http2;
  server_name sync.example.com;

  ssl_certificate /etc/letsencrypt/live/sync.example.com/fullchain.pem;
  ssl_certificate_key /etc/letsencrypt/live/sync.example.com/privkey.pem;

  client_max_body_size 110m;

  location / {
    proxy_pass http://127.0.0.1:6710;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
  }
}
```

### Traefik

The repository includes a Traefik example at
`deploy/traefik/docker-compose.traefik.yml`. Set `trusted_proxies` to the Docker
network CIDR used by Traefik, and replace the example domain and ACME email.

## trusted_proxies

Only trust `X-Forwarded-For` from your reverse proxy. If the proxy and app run
on the same host:

```toml
[network]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
```

If using Docker bridge networking:

```toml
[network]
trusted_proxies = ["172.16.0.0/12"]
```

Do not add broad public ranges. A client that can spoof `X-Forwarded-For`
weakens rate-limit and audit data.

## Runtime Security Settings

Review these from the Admin WebUI:

- Registration mode: keep `disabled` or `invite_only` for private deployments.
- Login rate-limit threshold, window, and lock duration.
- Maximum file size, default `100 MiB`.
- Supported text extensions.
- Timezone, default `Asia/Shanghai`.

Registration and login failures are rate limited. Admin-created users and CLI
users still need strong passwords.

Authenticated sync API routes are also fixed-window rate limited at 600
requests per 60 seconds per route, method, client IP, and bearer token. Keep
`trusted_proxies` accurate so the limiter and audit log see the real client IP.

## Prometheus Metrics

`/metrics` is disabled by default. When the `enable_metrics` runtime setting is
true, the endpoint returns Prometheus text exposition and still requires every
production gate: deployment key middleware, plugin User-Agent guard, and an
admin bearer token.

Configure scrape clients to send `X-PKVSync-Deployment-Key`, an accepted
PKV Sync User-Agent, and `Authorization: Bearer <admin-token>`. Do not expose
metrics to unauthenticated networks.

## Backups

Back up these together:

- `/var/lib/pkv-sync/metadata.db`
- `/var/lib/pkv-sync/vaults/`
- `/var/lib/pkv-sync/blobs/`
- `/etc/pkv-sync/config.toml`

Use SQLite online backup or stop the service before copying the database. Keep
the database, Git vault repositories, and blobs from the same point in time when
possible.

Example with restic:

```bash
restic -r sftp:user@backup.example.com:/repo backup /var/lib/pkv-sync /etc/pkv-sync
```

Encrypt backups before they leave the machine and test restores periodically.

## Disk Encryption

Use LUKS, BitLocker, FileVault, or provider-managed disk encryption where
available. If your VPS provider cannot encrypt the root disk, encrypted offsite
backups become mandatory rather than optional.

## Token Hygiene

Device bearer tokens renew on authenticated use, expire after 90 idle days, and
can be revoked by users or administrators. Treat active tokens as credentials
until they expire or are revoked.

Obsidian stores the plugin's active token and deployment key in the vault-local
plugin data file, `<vault>/.obsidian/plugins/pkv-sync/data.json`. Tell users to
keep that file out of shared archives, untrusted sync targets, and plaintext
backups. Revoke the affected device token if the file may have leaked.

Recommended practice:

- Revoke lost devices from the Admin WebUI device pages.
- Prefer revoking a single lost device token over resetting the whole account.
- Rotate user passwords when credential compromise is suspected.
- Review old and revoked tokens during routine maintenance.

## Activity and Logs

PKV Sync records sync, vault lifecycle, and read-only browsing activity with
user, vault, action, device name, file count, size, IP, User-Agent, details,
and timestamp. Vault lifecycle rows include `create_vault` and `delete_vault`
from Admin WebUI, plugin, or API operations. Use the Admin WebUI activity
filters to inspect users or action types.

Watch application and reverse-proxy logs for repeated:

- `401`: invalid or expired credentials
- `403`: disabled account or forbidden operation
- `404`: rejected deployment key/User-Agent in production middleware
- `409`: sync head mismatch or duplicate resource
- `429`: login, registration, authenticated sync API, or MCP HTTP rate limit

## Release Hygiene

Before upgrading production:

1. Read `CHANGELOG.md`.
2. Verify the release tag matches server, plugin, OpenAPI, Docker, and docs
   versions.
3. Check the GitHub release contains Linux amd64, Linux arm64, Windows x64,
   plugin zip, and `SHA256SUMS`.
4. Verify the GHCR image exists for the tag and `latest`.
5. Back up current data.
6. Run migrations with the new binary.

Migrations are append-only once released. Do not squash published migrations for
an existing deployment.
