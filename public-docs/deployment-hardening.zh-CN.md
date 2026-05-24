# PKV Sync 部署加固指南

[English](./deployment-hardening.md) | 简体中文 | [繁體中文](./deployment-hardening.zh-Hant.md) | [日本語](./deployment-hardening.ja.md) | [한국어](./deployment-hardening.ko.md)

本文假设部署对象是自己、家庭、团队或可信朋友使用的小型自托管服务。PKV Sync 运维上比较简单，但服务端会保存可读的仓库内容，因此主机和备份卫生很重要。

## 威胁模型

PKV Sync 不提供端到端加密。仓库内容安全依赖多层控制：

1. HTTPS 传输加密
2. 部署密钥预认证
3. 用户名/密码登录和使用时续期的 bearer 设备 token
4. 按用户和笔记库执行授权检查
5. Admin session 和 CSRF 保护
6. 操作系统或云厂商磁盘加密
7. 最小化暴露服务
8. 加密且经过恢复测试的备份

请把服务端管理员和服务端文件系统视为可以访问仓库明文内容的可信边界。

## 推荐拓扑

```text
Internet -> HTTPS reverse proxy -> 127.0.0.1:6710 pkvsyncd
```

除非你有明确的额外网络控制层，否则不要把 `pkvsyncd` 直接暴露到公网。

## 安装前准备

准备：

- 域名，例如 `sync.example.com`
- 通过 `pkvsyncd genkey` 生成的部署密钥
- `/etc/pkv-sync/config.toml`
- 持久化数据目录，通常是 `/var/lib/pkv-sync`
- 带有效 TLS 证书的反向代理

服务端分享 URL 形式如下：

```text
https://sync.example.com/k_xxx/
```

请保持私密。部署密钥是 API 流量的预认证入口，但不能替代用户密码。

## 系统用户

```bash
sudo useradd --system --home /var/lib/pkv-sync --shell /usr/sbin/nologin pkv-sync
sudo mkdir -p /var/lib/pkv-sync /etc/pkv-sync
sudo chown -R pkv-sync:pkv-sync /var/lib/pkv-sync
sudo chmod 750 /var/lib/pkv-sync
```

将 `config.toml` 放在 `/etc/pkv-sync/config.toml`，并确保只有服务用户和管理员可以读取。

## 防火墙

典型主机只暴露 SSH 和 HTTPS：

```bash
sudo ufw allow OpenSSH
sudo ufw allow 443/tcp
sudo ufw enable
```

如果 Caddy 或其他 ACME HTTP-01 客户端管理证书，还需要开放 `80` 端口用于验证和跳转：

```bash
sudo ufw allow 80/tcp
```

宿主机直接运行时，让 `pkvsyncd` 只监听本机：

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

Docker Compose 中让应用监听容器所有接口；如果需要宿主机调试，只把宿主机端口发布到 localhost：

```toml
[server]
bind_addr = "0.0.0.0:6710"
```

```yaml
ports:
  - "127.0.0.1:6710:6710"
```

## Docker Compose + Caddy

如果希望由 Caddy 自动申请和续期 HTTPS 证书，使用这个路径。

1. 将 DNS 指向服务器：

   ```text
   sync.example.com A    <服务器 IPv4>
   sync.example.com AAAA <服务器 IPv6，可选>
   ```

2. 在 `docker-compose.yml` 同目录创建 `config.toml`：

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

3. 替换 `deploy/caddy/Caddyfile` 中的 `sync.example.com`。
4. 启动：

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

5. 全新数据库首次启动后，打开 setup wizard 创建第一个管理员账号：

   ```text
   https://sync.example.com/setup
   ```

   如条件允许，请把 setup 阶段放在私网或临时反向代理 allowlist 后完成，完成后立刻收紧公网访问。日常管理员登录使用 `https://sync.example.com/admin/login`。

备份 `./data`、`config.toml` 和 Caddy 的命名卷。

升级：

```bash
docker compose pull
docker compose up -d
docker compose logs -f pkv-sync
```

仪表盘默认每 24 小时检查一次 GitHub release；发现新版本时会显示提示。离线部署可用 `[update_check] enabled = false` 关闭。

## public_host(admin POST 必备)

把 `[server].public_host` 设置为运维实际访问 admin 面板使用的外部主机名（不含协议，必要时含端口），例如 `sync.example.com` 或 `pkv.local:8443`。admin CSRF 检查依据该值计算期望 Origin。配置 `public_host` 后，期望 Origin 固定为 `https://<public_host>`；反向代理传入的 `X-Forwarded-Proto` 不会把 admin CSRF 校验降级到后端 HTTP。

如果 `public_host` 留空,所有 admin POST 都会被拒绝,返回 `403 csrf validation failed`,并打一条 `tracing::warn` 日志。这是**有意的 fail-closed 行为**:另一种做法(回退请求自带的 `Host` 头)会把鉴权耦合到攻击者可影响的 header,且在代理转发不一致的 Host 时会出错。

`public_host` 同时驱动:

- 生产风格的 admin cookie(设置后启用 `Secure`、`SameSite=Strict`)
- admin "分享服务端 URL" 链接使用 `https://` 前缀
- `/api/plugin-manifest` 返回的插件资源 URL 使用 `https://` 外部主机

插件清单 URL 生成不会信任客户端传入的 `X-Forwarded-Proto`。生产环境请设置 `public_host`，这样插件自更新拿到的资源 URL 才会稳定指向真实外部主机。

对 SSE 来说,该设置也能帮反向代理识别长连接事件流而不是普通短请求。

## 安全响应头

PKV Sync 会在生产服务端栈里添加这些响应头:

- `X-Frame-Options: DENY`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: same-origin`
- `Content-Security-Policy`，包含 `frame-ancestors 'none'`、`default-src 'self'`、`object-src 'none'`、`form-action 'self'` 以及自托管图片／样式
- 在配置了 `public_host` 时添加 `Strict-Transport-Security`

请让 TLS 终止和 `public_host` 保持一致。只有当服务端被配置为 HTTPS 对外发布时，才会发送 HSTS。

## 反向代理注意事项

### Caddy

```caddyfile
sync.example.com {
  reverse_proxy 127.0.0.1:6710
}
```

### Nginx

仓库提供了 `deploy/nginx/pkv-sync.conf`。它会把 HTTP 跳转到 HTTPS，设置 `client_max_body_size 110m`，并转发 PKV Sync 用于 Host 和客户端 IP 处理的 header。

最小结构：

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

仓库在 `deploy/traefik/docker-compose.traefik.yml` 提供了 Traefik 示例。请把 `trusted_proxies` 设置为 Traefik 使用的 Docker 网络 CIDR，并替换示例域名和 ACME 邮箱。

## trusted_proxies

只信任来自反向代理的 `X-Forwarded-For`。如果代理和应用运行在同一台主机：

```toml
[network]
trusted_proxies = ["127.0.0.1/32", "::1/128"]
```

如果使用 Docker bridge 网络：

```toml
[network]
trusted_proxies = ["172.16.0.0/12"]
```

不要加入宽泛公网网段。如果客户端可以伪造 `X-Forwarded-For`，限流和审计数据都会变弱。

## 运行时安全设置

从 Admin WebUI 检查这些设置：

- 注册模式：私有部署建议保持 `disabled` 或 `invite_only`。
- 登录限流阈值、窗口和锁定时长。
- 认证同步 API 限流：按路由、方法、客户端 IP 和 bearer 设备 token 分桶，每 60 秒最多 600 次请求。
- 最大文件大小，默认 `100 MiB`。
- 支持的文本扩展名。
- 时区，默认 `Asia/Shanghai`。

注册和登录失败会被限流。管理员创建的用户和 CLI 用户仍应使用强密码。

## 指标端点

`/metrics` 只有在运行时设置中启用 `enable_metrics=true` 后才可用。即使启用，它也必须同时通过部署密钥中间件、插件 User-Agent guard 和管理员 bearer token，不应绕过反向代理或额外暴露到公网。

## 备份

一起备份：

- `/var/lib/pkv-sync/metadata.db`
- `/var/lib/pkv-sync/vaults/`
- `/var/lib/pkv-sync/blobs/`
- `/etc/pkv-sync/config.toml`

复制数据库时使用 SQLite 在线备份，或先停止服务。尽量让数据库、Git 笔记库和 blobs 来自同一时间点。

restic 示例：

```bash
restic -r sftp:user@backup.example.com:/repo backup /var/lib/pkv-sync /etc/pkv-sync
```

备份离开机器前应先加密，并定期测试恢复。

## 磁盘加密

尽量使用 LUKS、BitLocker、FileVault 或云厂商托管磁盘加密。如果 VPS 提供商无法加密根磁盘，加密离线备份就不是可选项，而是必要项。

## Token 管理

设备 bearer token 会在认证请求时续期，连续 90 天未使用才会过期，也可以由用户或管理员撤销。在过期或撤销前，请把活跃 token 当作凭据处理。

Obsidian 会把插件的活跃 token 和部署密钥保存在笔记库本地插件数据文件 `<vault>/.obsidian/plugins/pkv-sync/data.json` 中。请提醒用户不要把该文件放进共享压缩包、不可信同步目标或明文备份里。如果怀疑文件已经泄露，请撤销受影响的设备 token。

建议：

- 从 Admin WebUI 设备页面撤销丢失设备。
- 如果只丢失单台设备，优先撤销该设备 token，而不是重置整个账号。
- 怀疑账号凭据泄露时再轮换用户密码。
- 例行维护时检查旧 token 和已撤销 token。

## 活动和日志

PKV Sync 会记录 push、pull、create_vault 和 delete_vault 活动，包括用户、笔记库、设备名、文件数、大小、IP、User-Agent、详情和时间戳。`create_vault` 和 `delete_vault` 来自管理面板、插件和 API 的笔记库创建／删除操作。可以用 Admin WebUI 的活动筛选检查用户或操作类型。

关注应用和反向代理日志中重复出现的：

- `401`：凭据无效或已过期
- `403`：账号禁用或操作被禁止
- `404`：生产中间件拒绝部署密钥或 User-Agent
- `409`：同步 head 不匹配或资源重复
- `429`：登录、注册、认证同步 API 或 MCP HTTP 限流

## 发布卫生

生产升级前：

1. 阅读 `CHANGELOG.md`。
2. 确认 release tag 与服务端、插件、OpenAPI、Docker 和文档版本一致。
3. 检查 GitHub release 包含 Linux amd64、Linux arm64、Windows x64、插件 zip 和 `SHA256SUMS`。
4. 确认 GHCR 镜像存在对应 tag 和 `latest`。
5. 备份当前数据。
6. 如果当前部署是 0.x，启动 1.0 二进制或镜像前先阅读 [`upgrade-notes-v1.0.zh-CN.md`](./upgrade-notes-v1.0.zh-CN.md)。不要把 1.0 直接指向已有的 0.x `metadata.db`。
7. 二进制部署先运行 `pkvsyncd upgrade --dry-run` 预览 release 资产，再运行 `pkvsyncd upgrade --yes` 下载并校验当前二进制旁边的 `pkvsyncd.new`。只有校验通过后，才停止服务并替换二进制。
8. Docker 或 Kubernetes 部署应拉取或修改镜像 tag 并重启服务或 rollout，不要在容器内替换二进制。
9. 用新二进制或新镜像运行 migrations。

PKV Sync 1.0 使用单个 v1 SQLite 基线。在这次基线之后，已发布的 1.x migration 对已有 1.x 部署保持追加式。
