# 升级说明：0.x 到 1.0

[English](./upgrade-notes-v1.0.md) | 简体中文 | [繁體中文](./upgrade-notes-v1.0.zh-Hant.md) | [日本語](./upgrade-notes-v1.0.ja.md) | [한국어](./upgrade-notes-v1.0.ko.md)

文档版本：v1.1.0。

PKV Sync 1.0 是第一个稳定版。它也为后续 1.x 维护重置了 SQLite migration 基线。

## 重要数据库说明

PKV Sync 1.0 只发布一个 `0001_initial.sql` 基线 migration。由 0.x 版本创建的
SQLite 数据库**不支持原地升级**到 1.0.0。

如果你正在运行 0.x 服务端，请选择下面路径之一：

1. 旧部署只在迁移准备期间停留在最终 0.8.x patch 版本，用于备份、materialize 或导出数据。
2. 先备份或 materialize 每个笔记库，使用全新的 1.0 数据目录启动服务，重新创建用户和笔记库，然后把笔记库内容导入或 push 到新服务端。
3. 在任何迁移演练前，先用 `pkvsyncd backup` 保存完整的 0.x 数据根目录。

不要把 1.0 二进制或 Docker 镜像直接指向已有的 0.x `metadata.db`。

## 1.0 稳定承诺

从 1.0 开始，以下表面遵循语义化版本：

- `public-docs/openapi.yaml` 中记录的公开 REST 路由。
- MCP how-to 中记录的 MCP stdio 和 Streamable HTTP 工具行为。
- 面向 1.x 全新数据库的 SQLite migrations；在这次 v1 基线之后，未来 1.x migration 保持追加式。
- 每笔记库 git 仓库布局和内容寻址 blob 存储。
- CLI 子命令和已有参数。
- Obsidian 插件设置和同步行为，允许 1.x 正常添加向后兼容功能。

OpenAPI 中没有记录的路由，例如 Admin Web UI 表单处理器，属于内部实现细节。

## 推荐的 0.x 到 1.0 流程

1. 如条件允许，先把旧部署升级到最终 0.8.x patch 版本，然后仅用它完成备份、materialize 或导出准备。
2. 运行 `pkvsyncd backup --output <backup-dir>` 并妥善保存备份。
3. 对每个笔记库，使用最新 Obsidian 客户端、`git clone`，或
   `pkvsyncd materialize <vault-id> --output <dir>` 得到当前文件树。
4. 停止旧服务端。
5. 使用全新的空 `data_dir` 和 `metadata.db` 启动 PKV Sync 1.0。
6. 完成 `/setup`，重新创建用户和笔记库，然后 push 或导入 materialized 笔记库内容。
7. 通知用户把 Obsidian 插件更新到 1.0.0。

## 插件兼容性

1.0 服务端的受支持插件是随服务端捆绑的 1.0 Obsidian 插件。旧的 v0.8.x 插件使用同一套核心同步 API，但新的修复和自更新加固只在 1.0+ 中维护。

## 相对 0.x 的破坏性变化

- 由于 migrations 已压缩为单个 v1 基线，0.x SQLite 数据库不能原地升级。
- 首次运行 setup 仍然通过浏览器完成；全新服务端不会再把随机管理员密码打印到日志。

笔记内容、git 历史和 blobs 仍可通过 backup/materialize/recreate/import 工作流带到新部署。

## 已知注意事项

- 原生 per-vault E2EE 不属于 1.0 范围。今天需要客户端侧文件内容加密的用户可以使用
  [`git-crypt`](./git-crypt-howto.zh-CN.md)，并接受路径仍为明文的取舍。
- `/metrics` 默认关闭；启用后仍需生产认证门禁。
- 生产部署请配置 `public_host`。当服务端无法确定配置好的 HTTPS 公网 origin 时，admin POST 会故意 fail closed。
