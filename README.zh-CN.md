# PKV Sync

自托管 Obsidian 笔记库同步：Rust 服务端、SQLite 元数据、Git 文本历史、内容寻址附件存储，以及桌面／移动端 Obsidian 插件。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

[English](./README.md) | 简体中文

## 状态

PKV Sync 是 pre-1.0 软件。API、存储布局、发布形式和默认值都可能继续变更。

PKV Sync **不提供**端到端加密。服务端可以读取同步的笔记内容和附件。生产部署务必启用 HTTPS、严格的账号控制、加密磁盘、加密备份和主机层加固——详见 [部署加固指南](./public-docs/deployment-hardening.zh-CN.md)。

## 亮点

- **多用户、多笔记库** Obsidian 同步，带按笔记库 push 锁和幂等 push。
- **实时推送**——通过 Server-Sent Events 投递提交事件，小文本变更（≤ 8 KiB）直接内联在事件里，插件无需再 pull。健康网络下端到端目标亚秒级，轮询作为安全兜底保留。
- **Git 原生**——每个笔记库在磁盘上就是一个裸 git 仓库。每文件历史、unified diff、单文件恢复在插件和管理面板里都可见。可选启用只读 `git clone https://_:<token>@host/git/<vault>` 用于离线浏览或外部镜像。
- **AI 可读笔记库**——`pkvsyncd mcp` 通过 stdio 或无状态 Streamable HTTP 端点暴露只读 MCP 工具。
- **选择性 `.obsidian` 同步**——新笔记库默认带一组起步 allowlist，用于主题、snippet、快捷键、应用偏好、外观和启用插件列表。插件代码和插件设置仍需用户主动 opt-in。
- **冲突安全**——SSE 内联应用拒绝覆盖本地有未同步修改的文件；冲突落盘为保留原扩展名的 `.conflict-*` 文件，可在插件命令面板里用真 LCS 行差异预览并一键选择“保留本地”或“采纳远端”。
- **管理面板**——用户、设备 token、笔记库、邀请码、运行时设置、活动日志、blob 垃圾回收。响应式，中英双语。
- **安全**——Argon2id 密码哈希、原子化每 IP 登录速率限制（带突发保护）、未配置 `public_host` 时 CSRF fail-closed、无效密码与禁用账号统一返回“invalid credentials”、90 天 bearer 设备 token + 重新登录时自动轮换。
- **刻意简单**——单二进制、单 SQLite 元数据、每笔记库一个 bare git 仓库、每附件一个内容寻址 blob。**不**搞集群、不依赖 MySQL／PostgreSQL、不依赖 S3。
- 发布 Linux amd64／arm64、Windows x64 二进制以及多架构 GHCR Docker 镜像。

完整运维和用户操作请看 [管理员手册](./public-docs/admin-manual.zh-CN.md) 和 [用户手册](./public-docs/user-manual.zh-CN.md)。

## 存储布局

```text
data_dir/
  metadata.db        SQLite 元数据
  vaults/<vault-id>/ 每个远端笔记库的裸 Git 仓库
  blobs/<sha256>     内容寻址的二进制 blob
```

`metadata.db` 存储用户、笔记库、设备 token、邀请码、运行时设置、同步活动、blob 引用和幂等记录。Git 历史是版本化文件状态的事实源；blob 文件在被引用期间会保留，过宽限期后由 GC 清理。请用 `pkvsyncd backup` 快照数据根目录和对应的 `config.toml`。

## 发布资产

GitHub Release 提供：

- `pkvsyncd-x86_64-unknown-linux-gnu`
- `pkvsyncd-aarch64-unknown-linux-gnu`
- `pkvsyncd-x86_64-pc-windows-msvc.exe`
- `pkv-sync-plugin.zip`
- `SHA256SUMS`

Docker 镜像发布到 GHCR（多架构 `linux/amd64`、`linux/arm64`）：

```bash
docker pull ghcr.io/cyberkurry/pkv-sync:latest
docker pull ghcr.io/cyberkurry/pkv-sync:v0.5.0
```

## 快速开始：Docker Compose

这是推荐的部署路径。`deploy/caddy/` 里的 Caddy 通过 Let's Encrypt 申请并续签 HTTPS 证书；PKV Sync 只在 Compose 网络内的 `127.0.0.1:6710` 上监听，公网永远看不到明文 HTTP。

**前置条件**：服务器有指向自己的 DNS A/AAAA 记录，公网可达 `80` 和 `443`（80 端口用于 ACME HTTP-01 验证和 HTTP→HTTPS 跳转）。

1. **DNS 指向服务器**

   ```text
   sync.example.com A    <server IPv4>
   sync.example.com AAAA <server IPv6，可选>
   ```

2. **生成部署密钥**

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

3. **在 `docker-compose.yml` 同目录创建 `config.toml`**

   ```toml
   [server]
   bind_addr     = "0.0.0.0:6710"
   deployment_key = "k_replace_me_with_genkey_output"
   public_host   = "sync.example.com"   # admin POST 必备

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker 桥接网段

   [logging]
   level  = "info"
   format = "json"
   ```

   `public_host` **是关键字段**：不配置时 admin CSRF 检查会 fail-closed，所有 admin POST 都会被拒绝（详见部署加固指南）。

4. **编辑 `deploy/caddy/Caddyfile`**——把 `sync.example.com` 改成你的域名。compose 文件已经挂载好 Caddyfile 和 `caddy_data` 卷（用于 Let's Encrypt 证书持久化）。

5. **启动**

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

   首次启动会自动创建 `admin` 账号并把一次性密码打到 stderr——**立即记下来**。日志形如：

   ```text
   FIRST-RUN ADMIN CREATED
    username: admin
    password: <save this now>
   ```

6. **登录**

   打开 `https://sync.example.com/admin/login`，用 `admin` 登录、改密码，然后在 **Users → New** 创建你的第一个用户账号。

**数据落在哪儿**

- 服务端数据：`./data`（主机），挂到容器内 `/var/lib/pkv-sync`。维护前请用 `pkvsyncd backup` 生成快照。
- Caddy 证书：命名卷 `caddy_data`。
- 日志：`docker compose logs pkv-sync`（默认 JSON 格式）。

**升级**

```bash
docker compose pull
docker compose up -d
```

数据库迁移是 append-only 的，启动时自动应用。回滚的方式是从备份还原数据目录。

**生产加固**——详见 [部署加固指南](./public-docs/deployment-hardening.zh-CN.md)：反向代理细节（Caddy／Nginx／Traefik）、`trusted_proxies` 调优、`public_host` 语义、运行时 CSRF 行为、备份、磁盘加密、token 卫生。

## 服务端 CLI

```bash
pkvsyncd genkey                                      # 生成部署密钥
pkvsyncd -c /etc/pkv-sync/config.toml migrate up     # 应用迁移
pkvsyncd -c /etc/pkv-sync/config.toml serve          # 启动 HTTP 服务
pkvsyncd -c /etc/pkv-sync/config.toml user add alice [--admin]
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
pkvsyncd -c /etc/pkv-sync/config.toml materialize <vault-id> --output <dir>
pkvsyncd -c /etc/pkv-sync/config.toml backup --output <dir> [--data-dir <dir>] [--gzip]
pkvsyncd -c /etc/pkv-sync/config.toml restore --input <backup-dir> --data-dir <dir> [--force]
pkvsyncd -c /etc/pkv-sync/config.toml verify [--data-dir <dir>] [--no-fail]
```

默认配置路径：`/etc/pkv-sync/config.toml`。

`materialize` 会遍历某个笔记库的裸 git 树，把 blob pointer 文件展开为实际二进制内容——给 `git clone` 用户或离线检视用。

`backup` 使用 `VACUUM INTO` 快照 `metadata.db`，复制 `vaults/`、`blobs/` 和存在时的 `config.toml`，并写入带 pkvsyncd 版本、组件哈希、大小和数量的 `MANIFEST.json`。对停止运行的服务数据根目录做离线检查时可用 `--data-dir`；需要单个归档文件时可用 `--gzip`。

`restore` 会先检查 manifest 和组件哈希，再把数据复制回 `--data-dir` 指定的目标数据目录。目标目录非空时需要 `--force`，恢复完成后会自动运行 `verify`。

`verify` 会检查被引用的 blob 文件是否存在、内容 SHA-256 是否与文件名一致，报告孤立 blob，并用 `git2` 校验笔记库 git 仓库。缺失、损坏或 git 错误会返回失败；`--no-fail` 可覆盖退出码。

## Obsidian 插件

从 release 里下载 `pkv-sync-plugin.zip`，解压到 `<vault>/.obsidian/plugins/pkv-sync/`，在 Obsidian 设置里开启社区插件并启用 **PKV Sync**。从管理面板复制分享 URL（`https://sync.example.com/k_xxx/`），粘贴到插件、点 **Connect**，然后登录或注册并选择远端笔记库。

**本地文件就是事实源**。插件直接读写你正常的 Obsidian 笔记库目录——没有不透明的存储层，没有代理文件系统。插件设置和同步索引存在 Obsidian 的 `<vault>/.obsidian/plugins/pkv-sync/data.json` 里。

设备 token 90 天过期。同一设备重新登录会替换原来的活跃 token；不会保留多个并存的 stale token。

完整功能（命令面板、历史／diff modal、冲突解决、选择性同步规则、设备管理、语言和时区）详见 [用户手册](./public-docs/user-manual.zh-CN.md)。

## 配置

静态 `config.toml`（启动时读取）：

| 字段 | 用途 |
| --- | --- |
| `server.bind_addr` | 监听地址。反代后用 `127.0.0.1:6710`；Docker Compose 里用 `0.0.0.0:6710`。 |
| `server.deployment_key` | 由 `pkvsyncd genkey` 生成，客户端通过 `X-PKVSync-Deployment-Key` 头发送。 |
| `server.public_host` | 对外可见的主机名（必要时含端口）。**admin POST 必备**——详见部署加固指南。 |
| `storage.data_dir` | 数据根目录，包含 `metadata.db`、`vaults/`、`blobs/`。 |
| `storage.db_path` | SQLite 数据库路径（通常是 `<data_dir>/metadata.db`）。 |
| `network.trusted_proxies` | 允许设置 `X-Forwarded-For` / `X-Forwarded-Proto` 的 CIDR。 |
| `logging.level` | tracing filter，如 `info`、`debug`。 |
| `logging.format` | `json` 或 `pretty`。 |

运行时设置（注册模式、登录速率限制、最大文件大小、文本扩展名、push 去抖、SSE 内联内容上限、SSE 心跳、Git smart HTTP 开关、额外 exclude glob、历史／diff 功能开关）都在管理面板里编辑——详见 [管理员手册](./public-docs/admin-manual.zh-CN.md#运行时设置)。

## HTTP API

所有 `/api/*` 路由都要求部署密钥 header；认证路由还要求 bearer 设备 token。完整路由表、请求／响应 schema、SSE 事件 payload 格式见 [OpenAPI 规范](./public-docs/openapi.yaml)。

## 运维

- 使用 `pkvsyncd backup --output /var/backups/pkv/<date>` 生成快照。
- 定期运行 `pkvsyncd verify`，及时发现 SHA 漂移或孤立 blob。
- 使用 `pkvsyncd restore --input /var/backups/pkv/<date> --data-dir /var/lib/pkv-sync` 从快照恢复。
- 只有在目标数据目录可以先清空时，才对 `pkvsyncd restore` 使用 `--force`。
- 务必跑在 HTTPS 后；把 `[network].trusted_proxies` 限制到实际代理 CIDR。
- 关注日志里重复出现的 `401`、`403`、`409`、`429` 响应。
- 大量删除附件后从管理面板触发 blob 垃圾回收。
- 同步过程中断后，如果文件数／大小／blob 引用漂移，用管理面板的元数据 reconcile 修复。

## 文档

- [部署加固](./public-docs/deployment-hardening.zh-CN.md)
- [管理员手册](./public-docs/admin-manual.zh-CN.md)
- [用户手册](./public-docs/user-manual.zh-CN.md)
- [OpenAPI 规范](./public-docs/openapi.yaml)
- [Changelog](./CHANGELOG.md)

## 开发检查

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm --prefix plugin exec vitest run
npm --prefix plugin run typecheck
npm --prefix plugin run build
npm --prefix plugin run package
cargo build --release -p pkv-sync-server
pwsh -File scripts/ci-smoke.ps1
```

CI 在 Linux 和 Windows 上跑 Rust 格式化、Clippy 和测试；插件做 test／typecheck／build／package／audit；Docker 构建；以及发布二进制 smoke 测试。Release CI 还会额外构建 Linux amd64／arm64、Windows x64、插件包、多架构 Docker 镜像、checksum 和 GitHub release。

## License

AGPL-3.0-only。详见 [LICENSE](./LICENSE)。
