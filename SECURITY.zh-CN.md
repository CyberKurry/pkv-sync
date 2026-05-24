# 安全政策

[English](./SECURITY.md) | 简体中文 | [繁體中文](./SECURITY.zh-Hant.md) | [日本語](./SECURITY.ja.md) | [한국어](./SECURITY.ko.md)

## 支持版本

PKV Sync 从 v1.0.0 起遵循语义化版本。安全修复维护当前 minor 线和上一条
minor 线。

| 版本 | 状态 | 安全支持结束时间 |
| --- | --- | --- |
| 最新 1.x minor | 活跃支持 | 待定 |
| 上一个 1.x minor | 仅安全修复 | 下一条 1.x minor 发布时 |
| 0.x | 不支持 | v1.0.0 发布时 |

## 报告漏洞

请不要为安全漏洞创建公开 GitHub issue。

首选通道：通过 `cyberkurry/pkv-sync` 的 GitHub Security Advisories 提交私密报告。

请包含：

- 受影响的 PKV Sync 版本。
- 最小复现步骤。
- 影响评估。
- 如果你已经有建议修复方案，也请一并提供。

## 响应目标

- 初次确认：5 个工作日内。
- 严重性分级：10 个工作日内。
- 修复与协调披露：critical/high 级别 90 天内，medium/low 级别 180 天内。
- CVE：适用时通过 GitHub Security Advisories 分配。

## 范围

范围内：

- `pkvsyncd` 服务端二进制。
- Obsidian 插件。
- Admin Web UI。
- MCP stdio 和 Streamable HTTP transport。
- 推荐了不安全部署方式的公开文档。

范围外：

- PKV Sync 之外的主机、反向代理、TLS、Docker、systemd 和操作系统加固。
- 第三方 Obsidian 插件。
- 在 PKV Sync 使用方式下不可利用的依赖漏洞。
- 需要管理员权限或已经攻陷主机之后才能成立的报告。

## 已知非问题

- PKV Sync 1.0 默认把普通笔记库内容以明文形式存放在服务端，这是设计选择。原生
  per-vault E2EE 计划进入 1.x 路线图。今天就需要客户端侧加密的用户可以使用
  [`git-crypt`](./public-docs/git-crypt-howto.zh-CN.md)，并接受 README 中说明的取舍。
- `/metrics` 默认关闭。启用后，在生产 server stack 中仍然需要部署密钥中间件、
  被接受的 PKV Sync User-Agent 和管理员 bearer token。
- MCP HTTP 同时需要部署密钥和 bearer token 认证；是否公网暴露由运维者决定，但应像其他认证后的管理相邻入口一样保护。

## 披露

PKV Sync 遵循协调披露。除非报告者希望匿名，否则会在安全公告和 `CHANGELOG.md` 中致谢。
