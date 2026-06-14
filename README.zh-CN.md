# PKV Sync

**自托管你的 Obsidian 笔记库。** PKV Sync 跑在你自己的服务器上，把手机、平板、桌面端的 Obsidian 笔记库保持同步。一个二进制、一个 SQLite 数据库、每个笔记库一个 bare git 仓库——不需要集群，不需要 S3，不需要任何托管云。装好，让 Obsidian 连上去，笔记就同步了。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

文档版本：v1.4.1。

[English](./README.md) | 简体中文 | [繁體中文](./README.zh-Hant.md) | [日本語](./README.ja.md) | [한국어](./README.ko.md)

## 特性

- **多用户、多笔记库**同步，按设备签发令牌，每个笔记库带 push 锁与幂等重试。
- **实时推送**。小改动通过 SSE 在亚秒级落地；轮询作为兜底保险。
- **Git 即真相**。每个笔记库都是一个 bare git 仓库，单文件历史、统一 diff、单文件恢复开箱即用——插件端和管理后台都能用。
- **冲突安全**。插件不会静默覆盖本地改动，冲突会以 `.conflict-*` 文件呈现，一键「保留本地」或「采纳远端」。
- **五语言管理后台**（English、简中、繁中、日本語、한국어）：用户、设备令牌、笔记库、邀请码、活动日志、blob 垃圾回收，并对破坏性的笔记库和用户操作弹出确认。
- **AI 可读**。MCP 通过 stdio、独立 Streamable HTTP，或 `pkvsyncd serve` 内嵌的 `/mcp` 路由暴露读写工具。
- **默认有边界**。管理员创建/重置密码使用 setup 同级强密码策略；token 明文只展示一次；上传和 MCP 响应都有大小上限；实时 SSE 流会复查已撤销 token。
- **故意做得无聊**。单二进制、单 SQLite 元数据库、每库一个 bare git 仓、每个附件一个内容寻址 blob。

## 用 Docker Compose 快速上手

这是推荐路径。`deploy/caddy/` 里的 Caddy 通过 Let's Encrypt 自动签发 HTTPS；PKV Sync 在 compose 内网监听 `127.0.0.1:6710`，公网完全见不到明文 HTTP。

你需要：一个域名（比如 `sync.example.com`），其 A/AAAA 记录指向服务器；公网能访问到 `80` 和 `443` 端口（80 用于 ACME HTTP-01 验证）。

1. 生成部署密钥：

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. 在 `docker-compose.yml` 旁放一份 `config.toml`：

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
   embed_in_serve = false                # true 会在本服务上挂载 /mcp
   ```

3. 编辑 `deploy/caddy/Caddyfile`，把 `sync.example.com` 换成你的真实域名。

4. 启动整套服务：

   ```bash
   docker compose up -d
   ```

   浏览器打开 `https://sync.example.com/setup`，建第一个管理员账号。

5. 在 Obsidian 里把 `pkv-sync-plugin.zip` 解压到 `<vault>/.obsidian/plugins/pkv-sync/`，启用插件，从管理后台复制分享 URL 粘进去，登录或注册，选一个笔记库。

后续更新就是 `docker compose pull && docker compose up -d`。如果要原生安装、调反向代理（Caddy／Nginx／Traefik）、了解 `public_host` 的语义、做备份还原或磁盘加密，请看[部署加固指南](./public-docs/deployment-hardening.zh-CN.md)。

## MCP 部署模式

PKV Sync 提供两种 MCP Streamable HTTP 部署方式。内嵌模式需要显式开启：设置 `[mcp].embed_in_serve = true` 后，`pkvsyncd serve` 会在主服务端口挂载 `/mcp`，复用同一套 TLS 终止、反向代理、部署密钥和 bearer 令牌校验。独立模式保留原有单独进程：`pkvsyncd mcp --transport http --bind 127.0.0.1:6711`，适合隔离 MCP、专用监听地址或独立扩缩容。

## Obsidian 插件

本地文件就是真相——插件直接读写你磁盘上的 Obsidian 笔记库，不存在代理文件系统这种东西。非敏感的插件设置和同步索引保存在 `<vault>/.obsidian/plugins/pkv-sync/data.json`；登录状态、当前 bearer 设备令牌、部署密钥和稳定设备身份保存在 Obsidian 的设备本地存储中。请把 Obsidian 设备本地存储、明文备份以及旧版本留下的插件 `data.json` 副本当成敏感数据。设备令牌在使用时会自动续期，90 天无活动后失效，且单个令牌最长有效 365 天；在同一设备重新登录会轮换掉旧令牌。

日常使用——命令面板、文件历史、并排 diff、冲突解决、`.obsidian` 选择性同步、设备管理、插件自更新——都写在[用户手册](./public-docs/user-manual.zh-CN.md)里。

## 关于加密

PKV Sync 1.0 **暂不**提供原生端到端加密——服务端能读到笔记内容。原生的按库 E2EE 在 1.x 路线图上，将以可选模式上线，因为加密会换掉服务端那些让 Git-native PKV 真正有用的功能（历史 diff、三方自动合并、SSE 内联推送、MCP 读写）。

在原生 E2EE 落地前，如果你需要端到端加密，可以在笔记库上叠一层 [`git-crypt`](https://github.com/AGWA/git-crypt)：被标记的路径会以密文 blob 形式到达服务端，服务端无法解密。文件名仍以明文形式存在于服务端（对大多数威胁模型来说可接受）。持有密钥的客户端依然可以用标准 `git clone` 和 `pkvsyncd materialize`。

生产部署还应该跑在 HTTPS 后面、把 `trusted_proxies` 收紧、给数据盘加密、给备份加密——具体看[部署加固指南](./public-docs/deployment-hardening.zh-CN.md)。

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

PKV Sync 1.4.1 经一轮深度安全审查，全面加固部署与可靠性：容器镜像现内置运行时健康检查、固定镜像 tag、Caddy 安全头与请求体上限；异步 diff／历史／冲突弹窗在关闭后不再写入已分离 DOM；解决冲突后立即推送；恢复确认文本对文件名中的替换语法（如 $&／$’）不再损坏。

PKV Sync 1.0 是第一个稳定版。公开 REST API、CLI、存储布局、插件包、Docker 镜像作为一组同步发版，遵循 semver：1.X.Y 在公开表面保持向后兼容，OpenAPI 规范是这个兼容契约的权威来源。0.x 创建的 SQLite 库**不支持**就地升级到 1.0.0——请按 [1.0 升级说明](./public-docs/upgrade-notes-v1.0.zh-CN.md)操作。

每个 GitHub release 会发布 Linux amd64/arm64 二进制、Windows x64 二进制、多架构 GHCR Docker 镜像、Obsidian 插件 zip 包，以及 `SHA256SUMS`。

## 开发自检

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
npm --prefix plugin run typecheck
npm --prefix plugin exec vitest run
npm --prefix plugin run build
```

CI 在 Linux 和 Windows 上跑完整 Rust 矩阵，加上插件的 test／typecheck／build／package、Docker 构建，以及发布二进制的冒烟测试。

## 许可

AGPL-3.0-only。详见 [LICENSE](./LICENSE)。
