# PKV Sync

自托管 Obsidian 笔记库同步：Rust 服务端、SQLite 元数据、Git 版本历史，以及 Obsidian 插件。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

[English](./README.md) | 简体中文

## 状态

PKV Sync 仍处于 1.0 之前阶段。API、存储布局、发布包和运维默认值仍可能变化。

当前设计不提供端到端加密。服务端可以读取笔记库内容。正式部署时请使用 HTTPS、严格访问控制、磁盘加密和加密备份。

## 包含内容

- `pkvsyncd`：服务端守护进程和 CLI
- `pkv-sync`：Obsidian 桌面端/移动端插件
- 配置的数据目录下保存 SQLite 元数据
- 每个笔记库使用独立 Git 仓库保存文本版本历史
- 二进制附件使用内容寻址 blob 存储
- 管理后台支持用户、设备 token、笔记库、邀请码、运行时设置和同步活动
- Docker、Docker Compose、Caddy、CI、release 和公开文档示例

## 当前功能

| 领域 | 当前行为 |
| --- | --- |
| 同步 | 通过 Obsidian 插件支持多用户、多笔记库同步 |
| 文本历史 | 文本文件会提交到每个笔记库对应的 Git 历史 |
| 附件 | 二进制文件按 SHA-256 内容 hash 存储 |
| 冲突 | 冲突编辑会保留为 `.conflict-*` 文件 |
| 排除规则 | 不同步 `.obsidian/`、`.trash/` 和冲突文件 |
| 认证 | 部署密钥预认证，加用户密码和 90 天有效期的 bearer 设备 token |
| 设备 | 插件持久化稳定设备 ID；同一设备重新登录会替换旧的活跃 token |
| 管理 | 仪表盘、用户、响应式用户详情页、设备 token、笔记库、邀请码、设置、活动和 blob GC |
| 时间显示 | 管理后台和插件都支持 IANA 时区选择，默认 `Asia/Shanghai` |
| 可靠性 | 插件串行化状态写入，并在拉取中断后记录部分进度，减少重复冲突文件 |
| 可观测性 | 结构化日志，支持 `json` / `pretty` 输出和可配置日志级别 |
| 发布 | Linux amd64、Linux arm64、Windows x64、插件 zip、校验和、GHCR Docker 镜像 |

## 发布产物

GitHub Releases 会发布：

- `pkvsyncd-x86_64-unknown-linux-gnu`
- `pkvsyncd-aarch64-unknown-linux-gnu`
- `pkvsyncd-x86_64-pc-windows-msvc.exe`
- `pkv-sync-plugin.zip`
- `SHA256SUMS`

Docker 镜像发布到：

```bash
docker pull ghcr.io/cyberkurry/pkv-sync:latest
```

带版本号的 release 也会发布 `ghcr.io/cyberkurry/pkv-sync:<version>`。

## 快速开始：Docker Compose

如果希望 Caddy 自动申请和续期 HTTPS 证书，使用这个部署方式。Caddy 需要公开 `80` 和 `443` 端口；`80` 用于 ACME HTTP-01 验证和 HTTP 到 HTTPS 跳转。

1. 将 DNS 指向服务器：

   ```text
   sync.example.com A    <服务器 IPv4>
   sync.example.com AAAA <服务器 IPv6，可选>
   ```

2. 生成部署密钥：

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

3. 在 `docker-compose.yml` 同目录创建 `config.toml`：

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

4. 编辑 `deploy/caddy/Caddyfile`，把 `sync.example.com` 换成你的域名。

5. 启动：

   ```bash
   docker compose up -d
   docker compose logs -f pkv-sync
   ```

6. 保存服务端日志里打印的首次启动管理员密码。

7. 打开：

   ```text
   https://sync.example.com/admin/login
   ```

更多细节见[部署加固指南](./public-docs/deployment-hardening.zh-CN.md)。

## 快速开始：本地二进制

从源码构建：

```bash
cargo build -p pkv-sync-server
npm ci --prefix plugin
npm --prefix plugin run build
```

生成部署密钥：

```bash
./target/debug/pkvsyncd genkey
```

基于 [`config.example.toml`](./config.example.toml) 创建 `config.toml`，然后运行：

```bash
./target/debug/pkvsyncd -c config.toml migrate up
./target/debug/pkvsyncd -c config.toml serve
```

如果通过反向代理部署，建议让 `pkvsyncd` 只监听本机：

```toml
[server]
bind_addr = "127.0.0.1:6710"
```

首次启动时，`pkvsyncd` 会创建 `admin` 账号并打印一次性密码。

## 服务端 CLI

```bash
pkvsyncd genkey
pkvsyncd -c /etc/pkv-sync/config.toml migrate up
pkvsyncd -c /etc/pkv-sync/config.toml serve
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
```

默认配置路径是 `/etc/pkv-sync/config.toml`。

## Obsidian 插件

从 release 手动安装：

1. 下载 `pkv-sync-plugin.zip`。
2. 解压到 `<vault>/.obsidian/plugins/pkv-sync/`。
3. 在 Obsidian 中启用社区插件。
4. 启用 **PKV Sync**。
5. 粘贴管理后台提供的服务端分享 URL：

   ```text
   https://sync.example.com/k_xxx/
   ```

6. 登录，创建或选择远端笔记库，然后使用自动同步或 **Sync now**。

插件会在本地保存稳定设备 ID。同一设备退出登录后再次登录，会替换该设备旧的活跃 token，而不是留下多个活跃 token。设备 token 默认 90 天后过期；重新登录会获得新的 token。

如果拉取过程中断，已经成功写入的文件会记录进度；下次重试同步时，不会因为这些已完成写入反复生成冲突文件。

## 管理后台

在服务器上打开 `/admin/login`。当前管理后台包括：

- 仪表盘：CPU、内存、数据目录所在磁盘用量，以及人类可读运行时间
- 用户管理、响应式用户详情页和密码重置
- 设备 token 创建、列表、撤销和 90 天有效期
- 笔记库创建、删除后端存储、元数据修复和大小显示
- 邀请码管理
- 运行时设置：服务名、时区、注册模式和登录限流
- 活动表：时间、用户、操作、设备名、笔记库名/ID、IP 和 User-Agent
- Blob 垃圾回收触发

## 配置说明

- 默认服务端口：`6710`
- 默认时区：`Asia/Shanghai`
- 默认注册模式：`disabled`
- 默认最大文件大小：`100 MiB`
- 默认文本扩展名：`md`、`canvas`、`base`、`json`、`txt`、`css`
- `trusted_proxies` 控制哪些反向代理可以设置 `X-Forwarded-For`
- `public_host` 用于生产环境 admin cookie 和分享 URL 生成

## 文档

- [部署加固](./public-docs/deployment-hardening.zh-CN.md)
- [管理员手册](./public-docs/admin-manual.zh-CN.md)
- [用户手册](./public-docs/user-manual.zh-CN.md)
- [OpenAPI 规范](./public-docs/openapi.yaml)
- [更新日志](./CHANGELOG.md)

## 开发检查

```bash
cargo fmt --all -- --check
cargo clippy -p pkv-sync-server --all-targets -- -D warnings
cargo test -p pkv-sync-server
npm --prefix plugin test
npm --prefix plugin run typecheck
npm --prefix plugin run build
cargo build --release -p pkv-sync-server
pwsh -File scripts/ci-smoke.ps1
```

CI 会运行 Linux 和 Windows Rust 检查、插件测试/typecheck/build、Docker build 和 release 二进制 smoke test。

## 许可证

AGPL-3.0-only。见 [LICENSE](./LICENSE)。
