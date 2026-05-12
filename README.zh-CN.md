# PKV Sync

自托管 Obsidian 笔记库同步：Rust 服务端、SQLite 元数据、Git 文本历史、内容寻址附件存储，以及支持桌面端和移动端的 Obsidian 插件。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

[English](./README.md) | 简体中文

## 状态

PKV Sync 仍处于 1.0 之前阶段。API、存储布局、发布包和运维默认值仍可能变化。

PKV Sync 不提供端到端加密。服务端可以读取同步的笔记库内容和附件。正式部署时应使用 HTTPS、严格的账号权限、磁盘加密、加密备份和主机加固。

## 组件

- `pkvsyncd`：服务端守护进程和 CLI
- `pkv-sync`：Obsidian 桌面端和移动端插件
- SQLite 元数据数据库
- 数据目录下的每笔记库裸 Git 仓库
- 用于二进制附件的 SHA-256 内容寻址 blob 存储
- Admin WebUI：用户、设备 token、笔记库、邀请码、设置、活动和清理
- Docker、Docker Compose、Caddy、Nginx、Traefik、systemd、CI 和 release workflow 示例

## 当前功能

| 领域 | 当前行为 |
| --- | --- |
| 同步模型 | 通过认证设备支持多用户、多笔记库 Obsidian 同步 |
| 文本历史 | 文本文件提交到每个笔记库对应的 Git 历史 |
| 历史与差异 | Obsidian 可查看单文件历史和 unified diff；Admin WebUI 可只读浏览文件、历史和差异 |
| 单文件恢复 | Obsidian 可把某个历史版本写回本地文件，并由正常同步流程推送 |
| 附件 | 二进制文件保存为 SHA-256 blob，并从 Git pointer 文件引用 |
| 冲突处理 | 本地/远端冲突保留为生成的 `.conflict-*` 文件 |
| 冲突清理 | 插件设置页和命令面板可以列出或删除生成的冲突文件 |
| 排除规则 | 不同步 `.obsidian/`、`.trash/` 和生成的冲突文件 |
| 认证 | 部署密钥预认证，加用户名/密码登录和 90 天 bearer 设备 token |
| 设备 | 插件持久化稳定设备 ID；同一设备重新登录会替换旧的活跃 token |
| 注册 | 运行时模式：禁用注册、仅邀请码、开放注册 |
| 管理 | 响应式仪表盘、用户、用户详情、设备 token、笔记库、只读文件/历史/diff 浏览、邀请码、设置、活动和 blob GC |
| 活动 | push、pull、历史、diff 和提交查看活动记录，支持按用户/动作筛选，显示设备名、笔记库、IP、User-Agent 和详情 |
| 时间显示 | 管理后台和插件时间戳使用可选 IANA 时区，默认 `Asia/Shanghai` |
| 可读值 | 管理后台以可读单位显示时间、运行时长、持续时间、大小和笔记库统计 |
| 可靠性 | 插件状态读写串行化、拉取部分进度、幂等 push、按笔记库 push 锁 |
| 发布 | Linux amd64、Linux arm64、Windows x64、插件 zip、校验和、GHCR Docker 镜像 |

## 存储布局

配置的 `[storage].data_dir` 中保存服务端管理的状态：

```text
data_dir/
  metadata.db        SQLite 元数据
  vaults/<vault-id>/ 每个远端笔记库的裸 Git 仓库
  blobs/<sha256>     内容寻址二进制 blob
```

`metadata.db` 记录用户、笔记库、设备 token、邀请码、运行时设置、同步活动、blob 引用和幂等记录。每个笔记库的 Git 历史是版本化文件状态来源；blob 文件在仍被引用时会保留，并在宽限期后由垃圾回收清理。

备份时应把 `metadata.db`、`vaults/`、`blobs/` 和 `config.toml` 放在同一备份集合中。

## 发布产物

GitHub Releases 会发布：

- `pkvsyncd-x86_64-unknown-linux-gnu`
- `pkvsyncd-aarch64-unknown-linux-gnu`
- `pkvsyncd-x86_64-pc-windows-msvc.exe`
- `pkv-sync-plugin.zip`
- `SHA256SUMS`

Docker 镜像发布到 GHCR：

```bash
docker pull ghcr.io/cyberkurry/pkv-sync:latest
docker pull ghcr.io/cyberkurry/pkv-sync:v0.1.10
```

Release Docker 镜像支持 `linux/amd64` 和 `linux/arm64`。

## 快速开始：Docker Compose

如果希望 Caddy 自动申请和续期 HTTPS 证书，使用这个部署方式。Caddy 需要公网 `80` 和 `443` 端口；`80` 用于 ACME HTTP-01 验证和重定向。

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

6. 保存服务端日志里打印的首次管理员密码。

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

首次启动时，`pkvsyncd` 会创建 `admin` 账号并打印一次性密码。请立刻保存，然后通过 Admin WebUI 或 CLI 修改。

## 服务端 CLI

```bash
pkvsyncd genkey
pkvsyncd -c /etc/pkv-sync/config.toml migrate up
pkvsyncd -c /etc/pkv-sync/config.toml serve
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user add alice --admin
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

6. 点击 **连接**，然后登录或注册。
7. 创建或选择远端笔记库。
8. 使用自动同步或 **立即同步**。

插件设置页包含：

- 铺满 Obsidian 设置页的深色界面
- 语言选择：自动、English、简体中文
- 时区选择，默认 `Asia/Shanghai`
- 从分享 URL 解析服务器 URL 和部署密钥
- 登录/注册状态下可点 **修改服务器** 返回连接设置，并保留已输入内容
- 设备名称编辑和稳定本地设备 ID
- 登录、注册、退出登录、创建远端笔记库和选择笔记库
- 手动同步按钮
- 上次成功同步时间显示为相对时间，并可展开精确 `YYYY/MM/DD HH:MM:SS` 时间
- 冲突文件数量和一键删除生成的冲突文件
- 设备列表和当前设备标记
- 历史与差异界面开关

命令面板动作：

- 显示同步状态
- 刷新账号信息
- 立即手动同步
- 查看同步状态详情
- 查看文件历史
- 查看仓库历史
- 列出冲突文件
- 删除冲突文件

同步行为：

- 在防抖间隔后推送本地变更
- 定期轮询远端变更
- 在相关笔记库文件事件和窗口失焦时触发同步
- 连接后使用服务端返回的文本扩展名列表
- 写入本地前校验下载的二进制 blob hash
- 通过串行化数据存储保存插件设置和同步索引
- 如果拉取写入中途失败，会记录已经完成的部分进度，减少重试时重复生成冲突文件
- 恢复文件历史版本时，会先从服务端读取历史内容，写回本地笔记库，再由现有同步引擎作为普通修改推送

设备 token 默认 90 天后过期。同一设备重新登录会替换该设备旧的活跃 token，而不是留下多个活跃 token。

## Admin WebUI

在服务器上打开 `/admin/login`。Admin WebUI 包括：

- 仪表盘：CPU、内存、数据目录磁盘用量、运行时长、用户、笔记库和最近活动
- 响应式侧边栏、移动端抽屉导航和内置 Lucide 图标
- 用户列表、用户创建、用户详情页、密码重置、启用/禁用、管理员权限控制和用户级 token 管理
- 全局设备 token 页面，可列出、创建和撤销 token
- 笔记库卡片：所有者、文件数、大小、上次同步、元数据修复和删除操作
- 只读笔记库文件浏览器，包含文件预览、单文件历史时间线和 unified diff 查看器。Admin WebUI 不提供恢复、revert 或 rollback 控制。
- 邀请码创建、过期时间展示，以及删除未使用邀请码
- 运行时设置分为 General、Security、Sync & Storage、Network
- 登录限流设置
- 最大文件大小和支持的文本扩展名设置
- Blob 垃圾回收触发
- 活动日志，支持按用户和动作真实筛选
- 英文和简体中文管理后台语言选择

保护措施包括最后一个管理员保护、禁止自我禁用/自我删除、用户名校验、Argon2id 密码哈希、90 天设备 token 过期、token 撤销、admin 表单 CSRF 检查，以及 API 路由的部署密钥预认证。

## 配置说明

静态 `config.toml` 字段：

- `server.bind_addr`：服务监听地址；反向代理后通常为 `127.0.0.1:6710`，Docker Compose 中通常为 `0.0.0.0:6710`
- `server.deployment_key`：由 `pkvsyncd genkey` 生成
- `server.public_host`：可选，用于生成 HTTPS 分享 URL 和生产风格 admin cookie
- `storage.data_dir`：数据根目录，包含 `metadata.db`、`vaults/` 和 `blobs/`
- `storage.db_path`：SQLite 数据库路径
- `network.trusted_proxies`：允许设置 `X-Forwarded-For` 的代理 CIDR
- `logging.level`：tracing filter，例如 `info` 或 `debug`
- `logging.format`：`json` 或 `pretty`

存储在 SQLite 且可从 Admin WebUI 编辑的运行时设置：

- 服务名称
- 时区，默认 `Asia/Shanghai`
- 注册模式：`disabled`、`invite_only`、`open`
- 登录失败阈值、窗口和锁定时长
- 最大文件大小，默认 `100 MiB`
- 支持的文本扩展名，默认 `md`、`canvas`、`base`、`json`、`txt`、`css`
- 历史界面和 diff 端点功能开关，默认均为开启

## HTTP API

所有 `/api/*` 路由都需要部署密钥 header。认证路由还需要 bearer 设备 token。

主要路由组：

- `GET /api/health`
- `GET /api/config`
- `POST /api/auth/login`
- `POST /api/auth/register`
- `GET /api/me`
- `POST /api/me/password`
- `POST /api/me/logout`
- `GET /api/me/tokens`
- `DELETE /api/me/tokens/:id`
- `GET /api/vaults`
- `POST /api/vaults`
- `DELETE /api/vaults/:id`
- `POST /api/vaults/:id/upload/check`
- `POST /api/vaults/:id/upload/blob`
- `GET /api/vaults/:id/state`
- `POST /api/vaults/:id/push`
- `GET /api/vaults/:id/pull`
- `GET /api/vaults/:id/commits`
- `GET /api/vaults/:id/commits/:commit`
- `GET /api/vaults/:id/history?path=`
- `GET /api/vaults/:id/diff?from=&to=&path=`
- `GET /api/vaults/:id/files/*path`
- `/api/admin/*` 下的管理员 API 路由

Schema 见 [OpenAPI 规范](./public-docs/openapi.yaml)。

## 运维

- 将 `config.toml`、`metadata.db`、`vaults/` 和 `blobs/` 放在同一备份集合中。
- 通过 HTTPS 部署。仓库提供 Caddy、Nginx 和 Traefik 反向代理示例。
- 如果使用反向代理，`trusted_proxies` 只应设置为代理网络。
- 关注日志中重复出现的 `401`、`403`、`409` 和 `429` 响应。
- 大量删除附件后运行 blob 垃圾回收。
- 如果中断操作后文件数、大小或 blob 引用发生漂移，使用笔记库元数据修复。
- 发版时保持 release 产物、Docker 镜像、插件包、更新日志和版本号一致。

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
npm --prefix plugin run package
cargo build --release -p pkv-sync-server
pwsh -File scripts/ci-smoke.ps1
```

CI 会在 Linux 和 Windows 上运行 Rust 格式检查、Clippy 和测试；插件测试/typecheck/build/package/audit；Docker build；以及 release 二进制 smoke test。

Release CI 还会构建 Linux amd64、Linux arm64、Windows x64、插件包、多架构 Docker 镜像、校验和和 GitHub release。

## 许可证

AGPL-3.0-only。见 [LICENSE](./LICENSE)。
