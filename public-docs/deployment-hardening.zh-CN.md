# PKV Sync 部署加固指南

[English](./deployment-hardening.md) | 简体中文

本文假设部署对象是给自己、家庭成员或朋友使用的小型自托管服务。

## 威胁模型

PKV Sync v1 不提供端到端加密。仓库内容的安全性依赖于：

1. HTTPS 传输加密
2. 部署密钥预认证
3. 用户密码和设备 token 认证
4. 按用户和仓库执行的授权检查
5. 操作系统级磁盘加密
6. 尽量少暴露服务
7. 加密备份

## 推荐拓扑

```text
Internet -> 443 reverse proxy -> 127.0.0.1:6710 pkvsyncd
```

除非你有明确理由和额外的网络控制层，否则不要直接暴露 `pkvsyncd`。

## 系统用户

```bash
sudo useradd --system --home /var/lib/pkv-sync --shell /usr/sbin/nologin pkv-sync
sudo mkdir -p /var/lib/pkv-sync /etc/pkv-sync
sudo chown -R pkv-sync:pkv-sync /var/lib/pkv-sync
```

把 `config.toml` 放在 `/etc/pkv-sync/config.toml`，并确保只有服务用户和管理员可以读取。

## 防火墙

只暴露 SSH 和 HTTPS：

```bash
sudo ufw allow OpenSSH
sudo ufw allow 443/tcp
sudo ufw enable
```

让 `pkvsyncd` 只监听本机地址：

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

## 磁盘加密

可使用 LUKS、BitLocker、FileVault，或主机提供商支持的磁盘加密。如果 VPS 提供商无法加密根磁盘，请把加密离线备份视为必要要求。

## 反向代理示例

## 使用 Docker Compose 和 Caddy

如果你希望由 Caddy 自动申请和续期 HTTPS 证书，建议使用这条路径。Caddy
需要公开 `80` 和 `443` 两个端口：`80` 用于 ACME HTTP-01 证书校验和
HTTP 到 HTTPS 跳转，`443` 用于 HTTPS 流量。

1. 把 DNS 指向服务器：

   ```text
   sync.example.com A    <服务器 IPv4>
   sync.example.com AAAA <服务器 IPv6，可选>
   ```

2. 打开防火墙端口：

   ```bash
   sudo ufw allow 80/tcp
   sudo ufw allow 443/tcp
   ```

3. 在 `docker-compose.yml` 同级创建 `config.toml`：

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

4. 修改 `deploy/caddy/Caddyfile`，把 `sync.example.com` 替换成你的真实域名。

5. 启动：

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

6. 从日志中保存首次启动的管理员密码，然后打开：

   ```text
   https://sync.example.com/admin/login
   ```

Compose 文件会让 `pkv-sync` 在宿主机上只发布到 `127.0.0.1:6710`，便于本机
调试；Caddy 则通过 Compose 内部网络访问 `pkv-sync:6710`。

需要备份 `./data`、`config.toml` 和 Caddy 的命名卷。升级时拉取新镜像并重建容器：

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

使用 Docker labels，并把 `trusted_proxies` 设置为 Traefik 使用的 Docker 网络 CIDR。

## trusted_proxies

只信任来自反向代理的 `X-Forwarded-For`。如果代理和应用运行在同一台主机上：

```toml
[network]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
```

如果使用 Docker bridge 网络：

```toml
[network]
trusted_proxies = ["172.16.0.0/12"]
```

使用 Docker Compose 时，让应用监听容器内所有接口，同时把宿主机端口发布限制在 localhost：

```toml
[server]
bind_addr = "0.0.0.0:6710"
```

```yaml
ports:
  - "127.0.0.1:6710:6710"
```

## 备份

需要备份：

- `/var/lib/pkv-sync/metadata.db`，使用 SQLite 在线备份或停服务后复制
- `/var/lib/pkv-sync/vaults/`
- `/var/lib/pkv-sync/blobs/`
- `/etc/pkv-sync/config.toml`

请保护 `config.toml`，因为其中包含部署密钥。

使用 restic 的示例：

```bash
restic -r sftp:user@backup.example.com:/repo backup /var/lib/pkv-sync /etc/pkv-sync
```

备份离开机器前应先加密。

## 日志隐私

PKV Sync 会记录运行和安全事件，包括调试及处理滥用所需的客户端信息。与其他用户共用服务器时，请提前告知。
