# PKV Sync Deployment Hardening Guide

English | [简体中文](./deployment-hardening.zh-CN.md)

This guide assumes a small self-hosted deployment for yourself, family, or
friends.

## Threat Model

PKV Sync v1 does not use end-to-end encryption. Protecting vault contents
depends on:

1. HTTPS transport encryption
2. Deployment key pre-auth
3. User password and device token authentication
4. Authorization checks per user and vault
5. OS-level disk encryption
6. Minimal exposed services
7. Encrypted backups

## Recommended Topology

```text
Internet -> 443 reverse proxy -> 127.0.0.1:6710 pkvsyncd
```

Do not expose `pkvsyncd` directly unless you have an explicit reason and a
separate network control layer.

## System User

```bash
sudo useradd --system --home /var/lib/pkv-sync --shell /usr/sbin/nologin pkv-sync
sudo mkdir -p /var/lib/pkv-sync /etc/pkv-sync
sudo chown -R pkv-sync:pkv-sync /var/lib/pkv-sync
```

Store `config.toml` in `/etc/pkv-sync/config.toml` and keep it readable only by
the service user and administrators.

## Firewall

Expose only SSH and HTTPS:

```bash
sudo ufw allow OpenSSH
sudo ufw allow 443/tcp
sudo ufw enable
```

Bind `pkvsyncd` to localhost:

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

## Disk Encryption

Use LUKS, BitLocker, FileVault, or your host provider's disk encryption where
available. If your VPS provider cannot encrypt the root disk, treat encrypted
offsite backups as mandatory.

## Reverse Proxy Examples

## Docker Compose With Caddy

Use this path when you want Caddy to request and renew HTTPS certificates for
you. Caddy needs both public ports `80` and `443`: port `80` is used for ACME
HTTP-01 certificate validation and HTTP-to-HTTPS redirects, and port `443`
serves HTTPS traffic.

1. Point DNS at the server:

   ```text
   sync.example.com A    <server IPv4>
   sync.example.com AAAA <server IPv6, optional>
   ```

2. Open the firewall:

   ```bash
   sudo ufw allow 80/tcp
   sudo ufw allow 443/tcp
   ```

3. Create `config.toml` next to `docker-compose.yml`:

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

4. Edit `deploy/caddy/Caddyfile` and replace `sync.example.com` with your
   real domain.

5. Start the stack:

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

6. Save the first-run admin password from the logs, then open:

   ```text
   https://sync.example.com/admin/login
   ```

The Compose file intentionally keeps `pkv-sync` published only on
`127.0.0.1:6710` for host debugging while Caddy reaches it through the internal
Compose network as `pkv-sync:6710`.

Back up `./data`, `config.toml`, and Caddy's named volumes. Upgrade by pulling a
new image and recreating the containers:

```bash
docker compose pull
docker compose up -d
```

### Caddy

```caddyfile
sync.example.com {
  reverse_proxy 127.0.0.1:6710
}
```

### Nginx

```nginx
server {
  listen 443 ssl http2;
  server_name sync.example.com;

  ssl_certificate /etc/letsencrypt/live/sync.example.com/fullchain.pem;
  ssl_certificate_key /etc/letsencrypt/live/sync.example.com/privkey.pem;

  location / {
    proxy_pass http://127.0.0.1:6710;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
  }
}
```

### Traefik

Use Docker labels and set `trusted_proxies` to the Docker network CIDR used by
Traefik.

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

For Docker Compose, set the application bind address to all container
interfaces while keeping the host publish bound to localhost:

```toml
[server]
bind_addr = "0.0.0.0:6710"
```

```yaml
ports:
  - "127.0.0.1:6710:6710"
```

## Backups

Back up:

- `/var/lib/pkv-sync/metadata.db` using SQLite online backup or stopped-service copy
- `/var/lib/pkv-sync/vaults/`
- `/var/lib/pkv-sync/blobs/`
- `/etc/pkv-sync/config.toml`

Protect `config.toml` because it contains the deployment key.

Example with restic:

```bash
restic -r sftp:user@backup.example.com:/repo backup /var/lib/pkv-sync /etc/pkv-sync
```

Encrypt backups before they leave the machine.

## Log Privacy

PKV Sync logs operational and security events, including client information
needed for debugging and abuse response. Inform users who share the server.
