# PKV Sync

PKV Sync 是一个自托管的、Git 原生的 Obsidian 笔记库同步服务端。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

文档版本：v1.7.0。

[English](./README.md) · 简体中文 · [繁體中文](./README.zh-Hant.md) · [日本語](./README.ja.md) · [한국어](./README.ko.md)

## 功能

### 同步

- **多用户、多笔记库**同步，按设备 Token 认证，每个笔记库各有推送锁，重试幂等。
- **实时推送。** 小的改动通过 Server-Sent Events 在亚秒级送达；轮询作为兜底手段保留。
- **以 Git 为唯一事实来源。** 每个笔记库都是一个裸 git 仓库，因此单文件历史、统一 diff、单文件还原开箱即用——插件里和管理后台里都能用。
- **冲突不会毁数据。** 插件绝不静默覆盖本地改动；冲突以 `.conflict-*` 文件的形式浮现出来，配合一键解决器。

### 运维

- **管理后台**支持五种语言（English、简中、繁中、日本語、한국어），覆盖用户、设备 Token、笔记库、邀请码、活动日志和 blob 垃圾回收，对破坏性的笔记库操作与用户操作都会弹出确认对话框。
- **笔记库可被 AI 读取。** MCP 通过 stdio 或 `pkvsyncd serve` 内嵌的 `/mcp` 路由（默认开启）对外提供读写工具，并且服务端已收录于 [MCP Registry](https://registry.modelcontextprotocol.io)，名为 `io.github.cyberkurry/pkv-sync`。
- **为 LLM 维护的 wiki 而生。** 除了读写，agent 还能按 frontmatter 检索、精确分页、按层 lint 链接、找出“来源已过期”的页面，并在“dry run + 内容哈希守卫”之后才写入——参见 [LLM Wiki 操作指南](./public-docs/llm-wiki-howto.zh-CN.md)。
- **默认就有边界约束。** 管理员创建的密码走 setup 级强密码策略，Token 密文只展示一次，上传和 MCP 响应都有大小上限，常驻的 SSE 流会重新校验已撤销的 Token。

### 设计

- **刻意选用成熟技术。** 单个二进制文件、单个 SQLite 元数据库、每个笔记库一个裸 git 仓库、每个附件一个内容寻址 blob。

## 用 Docker Compose 快速开始

推荐走这条路。`deploy/caddy/` 里的 Caddy 通过 Let's Encrypt 在前面挡 HTTPS；PKV Sync 在 compose 网络内部监听 `127.0.0.1:6710`，永远不会从公网直接收到明文 HTTP。

你需要一个域名（例如 `sync.example.com`），其 A/AAAA 记录指向这台服务器，并且公网能访问 `80` 和 `443` 端口（80 端口用于 ACME HTTP-01 校验）。

### 部署整套服务

1. 生成部署密钥：

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. 在 `docker-compose.yml` 旁边放一份 `config.toml`：

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # 换成 genkey 的输出
   public_host    = "sync.example.com"   # 必填，管理后台的 POST 才能正常工作

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker 网桥网段

   [mcp]
   embed_in_serve = true                 # 默认值；设为 false 可关闭
   ```

3. 编辑 `deploy/caddy/Caddyfile`，把 `sync.example.com` 换成你的真实域名。

4. 启动整套服务：

   ```bash
   docker compose up -d
   ```

   在浏览器里打开 `https://sync.example.com/setup`，创建第一个管理员账号。

5. 在 Obsidian 里安装 `pkv-sync-plugin.zip`
   （装到 `<vault>/.obsidian/plugins/pkv-sync/`），启用它，从管理后台复制分享
   URL 粘贴进去，然后登录或注册并选择一个笔记库。

### 更新

```bash
docker compose pull && docker compose up -d
```

### 接下来读什么

原生安装、反向代理调优（Caddy / Nginx / Traefik）、`public_host` 的含义、备份与还原、磁盘加密，这些内容见[部署加固指南](./public-docs/deployment-hardening.zh-CN.md)。

## MCP 部署模式

MCP HTTP 只提供内嵌方式：`[mcp].embed_in_serve` 默认为 `true`，因此 `pkvsyncd serve` 会把 `/mcp` 挂载到主服务器端口上，共用同一套 TLS 终止、反向代理、部署密钥和 bearer Token 校验。设为 `embed_in_serve = false` 可以关闭它。独立的 `pkvsyncd mcp --transport http --bind` 进程已在本版本中移除——现有的独立部署请改为指向主服务器的 `/mcp`。本地 AI 工具也可以走 stdio 传输：`pkvsyncd mcp --vault <id>`。

## Obsidian 插件

本地文件是唯一事实来源——插件直接读写你磁盘上正常的 Obsidian 笔记库，不设代理文件系统。非敏感的插件设置和同步索引放在 `<vault>/.obsidian/plugins/pkv-sync/data.json`；登录状态、当前生效的 bearer 设备 Token、部署密钥和稳定的设备标识则放在 Obsidian 的设备本地存储里。请把 Obsidian 的设备本地存储、明文备份，以及旧版本遗留的插件 `data.json` 副本一并视为敏感数据。设备 Token 在使用时续期，闲置 90 天后过期，绝对有效期 365 天；在同一台设备上重新登录会轮换当前生效的 Token。

日常功能——命令面板、文件历史、并排 diff、冲突解决、`.obsidian` 选择性同步、设备管理和自动更新——在[用户手册](./public-docs/user-manual.zh-CN.md)里逐一讲解。

## 当前的加密状况

PKV Sync 1.0 **尚未**提供原生的端到端加密——服务器能够读取笔记库内容。按笔记库开启的原生 E2EE 已列入 1.x 路线图，将作为可选模式提供，因为加密会让服务器端那些让 Git 原生 PKV 真正有用的能力全部失效（历史 diff、三方自动合并、SSE 内联载荷、MCP 读写）。

如果在这项能力落地之前你就需要 E2EE，可以在笔记库上叠一层 [`git-crypt`](https://github.com/AGWA/git-crypt)：被标记的路径到达服务器时是不透明密文 blob，服务器无法解密。文件名在服务器上仍是明文（对多数威胁模型而言可以接受）。持有密钥的客户端依然可以用标准 `git clone` 和 `pkvsyncd materialize`。

真实部署还应额外做到：跑在 HTTPS 后面、收紧 `trusted_proxies`、加密数据盘、加密备份——参见[部署加固指南](./public-docs/deployment-hardening.zh-CN.md)。

## 文档

| 主题 | 文档 |
| --- | --- |
| 插件的日常使用 | [用户手册](./public-docs/user-manual.zh-CN.md) |
| 服务器管理与运行时设置 | [管理员手册](./public-docs/admin-manual.zh-CN.md) |
| 每一个 CLI 命令与参数 | [CLI 参考](./public-docs/cli-reference.zh-CN.md) |
| 从 0.x 升级到 1.0 | [1.0 升级说明](./public-docs/upgrade-notes-v1.0.zh-CN.md) |
| 反向代理、TLS、备份、加固 | [部署加固](./public-docs/deployment-hardening.zh-CN.md) |
| HTTP API 契约 | [OpenAPI 规范](./public-docs/openapi.yaml) |
| MCP 配置与工具列表 | [MCP 操作指南](./public-docs/mcp-howto.zh-CN.md) |
| 由 LLM 维护的 wiki 工作流 | [LLM Wiki 操作指南](./public-docs/llm-wiki-howto.zh-CN.md) |
| 从 Obsidian Sync 迁移 | [迁移指南](./public-docs/migrate-from-obsidian-sync.zh-CN.md) |
| 安全漏洞报告 | [SECURITY.md](./SECURITY.md) |
| 发布历史 | [CHANGELOG.md](./CHANGELOG.md) |

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

CI 在 Linux 和 Windows 上跑完整的 Rust 矩阵，外加插件的 test／typecheck／build／package、一次 Docker 构建，以及发布二进制的冒烟测试。

## 参与贡献

欢迎提 bug、提需求和发 Pull Request。除了修 bug 之外的事情，请先开 issue 讨论。构建与测试命令、提交与发布流程，以及 CI 会校验的文档规则，见 [CONTRIBUTING.md](./CONTRIBUTING.md)。

## 许可证

AGPL-3.0-only。见 [LICENSE](./LICENSE)。
