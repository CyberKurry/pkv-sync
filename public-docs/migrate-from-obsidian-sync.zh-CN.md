# 从 Obsidian Sync 迁移

[English](./migrate-from-obsidian-sync.md) | 简体中文 | [繁體中文](./migrate-from-obsidian-sync.zh-Hant.md) | [日本語](./migrate-from-obsidian-sync.ja.md) | [한국어](./migrate-from-obsidian-sync.ko.md)

文档版本：v1.4.3。

本文说明如何把已经使用 Obsidian Sync 的 Obsidian 笔记库当前文件导入到新的 PKV Sync 笔记库。

迁移只导入当前设备上现有的文件。它不会导入 Obsidian Sync 历史、远端版本历史、已删除文件历史或冲突元数据。PKV Sync 的历史会从创建新 PKV 笔记库的迁移提交开始。

迁移也不会禁用、卸载或修改 Obsidian Sync。确认 PKV Sync 结果之后，如果你想停止使用 Obsidian Sync，请在 Obsidian 中手动关闭。

## 开始之前

- 先等待 Obsidian Sync 在用于迁移的设备上完成同步。
- 迁移前手动备份整个笔记库文件夹。
- 如有可能，导入期间保持 Obsidian 关闭，或至少不要编辑文件。
- 先创建或确认目标 PKV Sync 服务端账号。

## 会导入什么

PKV Sync 会创建一个新笔记库，并把当前导入内容作为第一条 PKV 历史提交。

普通 Markdown 文件、附件和常规笔记库文件会被导入，除非它们命中 PKV Sync 的强制排除规则。

## 会跳过什么

导入器会跳过 Obsidian Sync 内部文件、PKV Sync 插件自身状态、操作系统垃圾文件和本地运行时文件，包括：

- `.obsidian/sync/`
- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.obsidian/plugins/pkv-sync/`（插件自身的设置和 token 存储仅保留在本地）
- `.trash/**`
- `.git/**`
- `.DS_Store`（macOS）
- `Thumbs.db`（Windows）
- `*.tmp`、`*.lock` 等临时文件
- 设备专属的工作区、缓存、回收站和临时文件

部分 `.obsidian` 配置文件之后可以通过按笔记库 `.obsidian` allowlist 同步。相关规则请阅读 `.obsidian` 配置同步指南。

## 迁移之后

在另一台设备上打开新的 PKV 笔记库，确认笔记和附件看起来正确。检查完成前，请保留手动备份。

如果你继续让 Obsidian Sync 和 PKV Sync 使用同一个文件夹，请谨慎修改文件。两个同步系统可能会同时操作同一批文件，而 PKV Sync 只会记录迁移提交之后收到的变更。
