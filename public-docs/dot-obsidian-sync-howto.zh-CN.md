# 跨设备同步 `.obsidian` 配置

[English](./dot-obsidian-sync-howto.md) | 简体中文 | [繁體中文](./dot-obsidian-sync-howto.zh-Hant.md) | [日本語](./dot-obsidian-sync-howto.ja.md) | [한국어](./dot-obsidian-sync-howto.ko.md)

文档版本：v1.1.1。

PKV Sync 默认避开隐藏路径。它提供按笔记库配置的 allowlist，让你可以选择性同步 `.obsidian` 配置文件，而不是同步整个 Obsidian 内部目录。

## 新笔记库默认同步什么

新笔记库会得到这组起步 allowlist：

- 主题：`.obsidian/themes/**`
- CSS snippets：`.obsidian/snippets/**`
- 快捷键：`.obsidian/hotkeys.json`
- 应用偏好：`.obsidian/app.json`
- 外观偏好：`.obsidian/appearance.json`
- 已启用社区插件列表：`.obsidian/community-plugins.json`
- 已启用核心插件列表：`.obsidian/core-plugins.json`

这里仅包含已启用插件列表。插件代码和插件设置默认不会同步。

已有笔记库会保持空 allowlist，直到你应用起步清单。

- **Admin WebUI：Vaults -> Settings -> Apply starter allowlist** 会写入上述完整的 7 条 glob 起步清单。
- **Obsidian 插件：Settings -> PKV Sync -> Apply recommended starter list** 只写入最安全的两条 glob（`.obsidian/themes/**` 和 `.obsidian/snippets/**`）——主题和 CSS snippet 跨设备共享通常是安全的，而另外五条 glob 涉及用户特定的应用状态，插件不会在没有明确决定的情况下启用它们。

如果想要完整的 7 条 glob 起步清单，请使用 Admin WebUI 按钮，或者把这些 glob 手动粘贴到插件的 allowlist 编辑器中。

## 永不同步

以下硬排除始终优先，即使你把它们加入 allowlist 也不会同步：

- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.git/**`
- `.trash/**`
- `.conflict-*`
- `*.lock`
- `*.tmp`

## 进阶 opt-in

你可以添加额外 glob，但需要自行承担风险：

- `.obsidian/plugins/*/data.json`：插件设置。这里可能包含 API key、OAuth token 或 LLM key。在端到端加密落地前，同步内容会以明文存放在服务端。
- `.obsidian/plugins/**`：插件代码。这会让 Git 历史快速膨胀，并且桌面专用插件同步到移动端时可能无法运行。
- 其他隐藏目录，例如 `.claude/**` 或 `.codex/**`：agent 状态可能包含敏感的本地上下文。

## 在哪里编辑规则

- Obsidian：打开 **设置 -> PKV Sync**，选择当前笔记库，编辑 **.obsidian 同步规则**，然后保存。
- Admin WebUI：打开 **Vaults**，点击某个笔记库的 **Settings**，编辑 allowlist，然后保存。
