# PKV Sync

自托管 Obsidian 仓库同步服务，带服务端版本历史。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

[English](./README.md) | 简体中文

## 这是什么

PKV Sync 是一套小型自托管 Obsidian 同步栈：

- `pkvsyncd`：Rust 服务端守护进程，使用 SQLite 保存元数据，使用 Git 保存仓库历史，并提供 HTTP API
- `pkv-sync`：Obsidian 桌面端和移动端插件

它适合你自己、家庭成员或小范围可信用户共用一台自托管服务器。

## 状态

预发布阶段。v1.0 前 API、存储布局和发布包格式都可能变化。

当前设计不提供端到端加密。正式部署时请使用 HTTPS、磁盘加密、严格访问控制和加密备份。

## 功能

| 领域 | 当前行为 |
| --- | --- |
| 仓库同步 | 通过 Obsidian 插件支持多用户、多仓库同步 |
| 版本历史 | 文本文件在服务端按仓库提交到 Git 历史 |
| 附件 | 二进制文件存入内容寻址 blob 存储 |
| 冲突 | 本地冲突编辑会保留为 `.conflict-*` 文件 |
| 管理 | 首次启动管理员初始化、管理后台、用户和邀请码管理 |
| 认证 | 部署密钥预认证加 bearer 设备 token |
| 存储 | SQLite 元数据，本地文件系统数据目录 |
| 运维 | 支持 systemd、反向代理、Docker Compose 和发布打包 |

## 快速开始（开发）

构建服务端和插件：

```bash
cargo build -p pkv-sync-server
npm install --prefix plugin
npm --prefix plugin run build
```

生成部署密钥：

```bash
./target/debug/pkvsyncd genkey
```

从 [`config.example.toml`](./config.example.toml) 创建 `config.toml`，然后运行：

```bash
./target/debug/pkvsyncd -c config.toml migrate up
./target/debug/pkvsyncd -c config.toml serve
```

首次启动时，`pkvsyncd` 会创建 `admin` 账号并打印一次性密码。

## 部署方式

| 方式 | 适用场景 |
| --- | --- |
| 二进制 + systemd + 反向代理 | 你想直接控制主机和数据目录 |
| Docker Compose + Caddy | 你想用简单的容器化部署 |
| 现有反向代理 | 你已经在使用 Caddy、Nginx、Traefik 或其他 TLS 终止服务 |

把服务暴露到公网前，请先阅读[部署加固指南](./public-docs/deployment-hardening.zh-CN.md)。

## Obsidian 插件

从 release 手动安装：

1. 下载 `pkv-sync-plugin.zip`。
2. 解压到 `<vault>/.obsidian/plugins/pkv-sync/`。
3. 在 Obsidian 中启用社区插件。
4. 启用 **PKV Sync**，粘贴管理员提供的服务器共享 URL。

共享 URL 形如：

```text
https://sync.example.com/k_xxx/
```

## 文档

- [部署加固指南](./public-docs/deployment-hardening.zh-CN.md)
- [管理员手册](./public-docs/admin-manual.zh-CN.md)
- [用户手册](./public-docs/user-manual.zh-CN.md)
- [OpenAPI 规范](./public-docs/openapi.yaml)
- [更新日志](./CHANGELOG.md)

## 开发检查

```bash
cargo fmt --check
cargo clippy -p pkv-sync-server -- -D warnings
cargo test -p pkv-sync-server
npm --prefix plugin test
npm --prefix plugin run typecheck
npm --prefix plugin run build
```

## 许可证

AGPL-3.0-only。见 [LICENSE](./LICENSE)。
