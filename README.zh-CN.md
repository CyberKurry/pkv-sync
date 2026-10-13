# PKV Sync

**自托管你的 Obsidian 笔记库。** PKV Sync 跑在你自己的服务器上，让手机、平板、桌面端的 Obsidian 笔记库保持同步。一个二进制、一个 SQLite 数据库、每个笔记库一个 bare git 仓库——不需要集群，不需要 S3，不需要任何托管云。装好，把 Obsidian 指过来，笔记就同步了。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

文档版本：v1.6.0。

[English](./README.md) · 简体中文 · [繁體中文](./README.zh-Hant.md) · [日本語](./README.ja.md) · [한국어](./README.ko.md)

## 特性

- **多用户、多笔记库。**通过认证的设备接入同步，每个笔记库带推送锁（push lock）和幂等重试。
- **实时推送。**小改动通过 Server-Sent Events（SSE）亚秒级落地；轮询作为兜底。
- **Git 即唯一事实来源。**每个笔记库都是一个 bare git 仓库，单文件历史、统一 diff、单文件还原开箱即用——插件端和管理后台都能用。
- **冲突安全。**插件不会静默覆盖本地改动，冲突会以 `.conflict-*` 文件呈现，一键“保留本地”或“采纳远端”。
- **五语言管理后台**（English、简中、繁中、日本語、한국어）：管理用户、设备令牌（device token）、笔记库、邀请码、活动日志和 blob 垃圾回收；对笔记库和用户的破坏性操作会弹出确认对话框。
- **AI 可读的笔记库。**MCP 通过 stdio 或 `pkvsyncd serve` 内嵌的 `/mcp` 路由（默认开启）暴露读写工具。
- **默认从严。**管理员创建的密码遵循 setup 同级的强密码策略，令牌明文仅展示一次，上传和 MCP 响应都有大小上限，实时 SSE 流会复查已撤销的令牌。
- **故意做得无聊。**单二进制、单 SQLite 元数据库、每库一个 bare git 仓、每个附件一个内容寻址 blob。

## 用 Docker Compose 快速上手

这是推荐路径。`deploy/caddy/` 里的 Caddy 通过 Let's Encrypt 负责 HTTPS 对外入口；PKV Sync 在 compose 内网监听 `127.0.0.1:6710`，公网的明文 HTTP 根本到不了它。

你需要一个域名（比如 `sync.example.com`），A/AAAA 记录指向服务器，并且公网能访问 `80` 和 `443` 端口（80 用于 ACME HTTP-01 验证）。

1. 生成部署密钥（deployment key）：

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. 在 `docker-compose.yml` 旁边放一份 `config.toml`：

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # 替换为 genkey 输出
   public_host    = "sync.example.com"   # 必填，管理端 POST 才能通

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker bridge 网段

   [mcp]
   embed_in_serve = true                 # 默认值；设为 false 可禁用
   ```

3. 编辑 `deploy/caddy/Caddyfile`，把 `sync.example.com` 换成你的真实域名。

4. 启动整套服务：

   ```bash
   docker compose up -d
   ```

   浏览器打开 `https://sync.example.com/setup`，创建第一个管理员账号。

5. 在 Obsidian 安装 `pkv-sync-plugin.zip`（解压到 `<vault>/.obsidian/plugins/pkv-sync/`），启用插件，粘贴管理后台里的分享 URL，登录或注册后选一个笔记库。

更新只需 `docker compose pull && docker compose up -d`。原生安装、反向代理调优（Caddy / Nginx / Traefik）、`public_host` 语义、备份还原和磁盘加密，请看[部署加固指南](./public-docs/deployment-hardening.zh-CN.md)。

## MCP 部署模式

MCP HTTP 仅内嵌提供：`[mcp].embed_in_serve` 默认为 `true`，`pkvsyncd serve` 会在主服务端口挂载 `/mcp`，复用同一套 TLS 终止、反向代理、部署密钥和 bearer 令牌校验。设置 `embed_in_serve = false` 可禁用。独立进程 `pkvsyncd mcp --transport http --bind` 已在本版本移除——原有的独立部署请改为指向主服务的 `/mcp`。本地 AI 工具也可以使用 stdio 传输：`pkvsyncd mcp --vault <id>`。

## Obsidian 插件

本地文件是唯一事实来源——插件直接读写你磁盘上的普通 Obsidian 笔记库，没有代理文件系统这种东西。非敏感的插件设置和同步索引保存在 `<vault>/.obsidian/plugins/pkv-sync/data.json`；登录状态、当前 bearer 设备令牌、部署密钥和稳定设备身份保存在 Obsidian 的设备本地存储中。请把 Obsidian 设备本地存储、明文备份以及旧版本留下的插件 `data.json` 副本当成敏感数据。设备令牌在使用时自动续期，闲置 90 天后失效，单个令牌最长存活 365 天；在同一设备重新登录会轮换掉旧令牌。

日常功能——命令面板、文件历史、并排 diff、冲突解决、`.obsidian` 选择性同步、设备管理、插件自更新——都写在[用户手册](./public-docs/user-manual.zh-CN.md)里。

## 加密现状

PKV Sync 1.0 **暂不**提供原生端到端加密——服务端能读到笔记内容。原生的按库 E2EE 在 1.x 路线图上，将以可选模式上线，因为加密会换掉服务端那些让 Git-native PKV 真正有用的功能（历史 diff、三方自动合并、SSE 内联推送、MCP 读写）。

在原生 E2EE 落地前需要端到端加密的话，可以在笔记库上叠一层 [`git-crypt`](https://github.com/AGWA/git-crypt)：被标记的路径会以密文 blob 的形式到达服务端，服务端无法解密。文件名在服务端仍是明文（对大多数威胁模型来说可接受）。持有密钥的客户端依然可以用标准 `git clone` 和 `pkvsyncd materialize`。

生产部署还应该跑在 HTTPS 后面、收紧 `trusted_proxies`、加密数据盘和备份——具体看[部署加固指南](./public-docs/deployment-hardening.zh-CN.md)。

## 你在找……

| 主题 | 文档 |
| --- | --- |
| 插件日常使用 | [用户手册](./public-docs/user-manual.zh-CN.md) |
| 服务端管理与运行时设置 | [管理员手册](./public-docs/admin-manual.zh-CN.md) |
| 所有 CLI 命令和参数 | [CLI 参考](./public-docs/cli-reference.zh-CN.md) |
| 从 0.x 升级到 1.0 | [1.0 升级说明](./public-docs/upgrade-notes-v1.0.zh-CN.md) |
| 反向代理、TLS、备份、加固 | [部署加固](./public-docs/deployment-hardening.zh-CN.md) |
| HTTP API 契约 | [OpenAPI 规范](./public-docs/openapi.yaml) |
| MCP 安装与工具列表 | [MCP 操作指南](./public-docs/mcp-howto.zh-CN.md) |
| LLM 维护的 Wiki 工作流 | [LLM Wiki 操作指南](./public-docs/llm-wiki-howto.zh-CN.md) |
| 从 Obsidian Sync 迁移 | [迁移指南](./public-docs/migrate-from-obsidian-sync.zh-CN.md) |
| 安全漏洞反馈 | [SECURITY.md](./SECURITY.md) |
| 发布记录 | [CHANGELOG.md](./CHANGELOG.md) |

## 状态

PKV Sync 1.6.0 让 MCP 成为一等公民：服务器同时支持新的 2026-07-28 无状态协议修订版和旧客户端（双时代），新增 `subscriptions/listen` 推送，并默认把 MCP 内嵌进主服务器（独立的 `mcp --transport http` 部署已移除）。无法设置 `public_host` 的管理部署获得了 `allow_insecure_admin` 逃生通道，健康探针修复后，自升级总算真正生效，不再静默回滚。同步正确性修复一并到位：auto-merge 不再跳过提交（本地头不再越过从未拉取的合并基点），实时应用也不再静默覆盖或复活本地文件。

PKV Sync 1.5.0 交付了 2026-08-16 全仓库复审的全部修复，外加一轮性能优化：blob 保留现在跟随 git 对象存活性，回滚与按文件历史还原的附件不再丢失；Windows 不安全的路径组件在进入 `materialize` 前就会被拒绝；特权更新器拒绝降级，Docker 自升级覆盖已加固；注册与 blob 上传路径补齐了缺失的限流；常驻 SSE 连接不再阻塞优雅关机；插件可同步 allowlist 内的 `.obsidian` 路径，并以内存有界的批次流式完成首次同步（面向移动端）；批量 git 读取加单次拷贝的索引更新，让 MCP 搜索和大型笔记库同步都更快。

PKV Sync 1.0 是第一个稳定版。公开 REST API、CLI、存储布局、插件包、Docker 镜像遵循同一套 semver 一起发版：1.X.Y 在公开表面保持向后兼容，OpenAPI 规范是这份兼容契约的权威来源。0.x 创建的 SQLite 库**不支持**就地升级到 1.0.0——请按 [1.0 升级说明](./public-docs/upgrade-notes-v1.0.zh-CN.md)操作。

每个 GitHub release 会发布 Linux amd64/arm64 二进制、Windows x64 二进制、多架构 GHCR Docker 镜像、Obsidian 插件 zip 包，以及 `SHA256SUMS`。

## 开发

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace

cd plugin
npm run typecheck
npm exec vitest run
npm run build
```

CI 在 Linux 和 Windows 上跑完整 Rust 矩阵，外加插件的测试、类型检查、构建、打包，Docker 构建，以及发布二进制的冒烟测试。

## 许可

AGPL-3.0-only。详见 [LICENSE](./LICENSE)。
