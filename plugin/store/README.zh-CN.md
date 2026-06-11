# PKV Sync

自托管、以 Git 为底座的 Obsidian 同步插件。

> 本仓库是 PKV Sync Obsidian 插件的**发布镜像**：包含插件源码与发布产物，由[主仓库](https://github.com/CyberKurry/pkv-sync)在每次发版时自动发布。本仓库不接受 PR——开发在主仓库进行；issue 在本仓库或主仓库提交均可。

[English](README.md)

## 这是什么

PKV Sync 通过**你自己托管**的服务器在多设备间同步笔记库。Git 是事实源：每次变更都是一个 commit，支持单文件历史、差异对比、恢复，以及笔记库级回滚。

## 亮点

- 多用户、多笔记库同步，亚秒级推送（SSE）
- 完整版本历史：单文件 diff／恢复，笔记库级回滚
- 文本并发编辑三路自动合并；剩余冲突生成可解决的 `.conflict-*` 文件，并提供应用内差异视图
- Git 原生访问：可用任意 Git 工具通过 HTTPS 以只读方式 `git clone` 你的笔记库
- 内置 MCP 服务器，你自己的 AI 智能体可以读取并维护你的笔记
- 管理后台 Web UI、Prometheus 指标、备份／恢复／校验 CLI
- 支持 English、简体中文、繁體中文、日本語、한국어
- 一键从 Obsidian Sync 迁移

## 使用前提

本插件需要**自托管的 PKV Sync 服务器**。服务器由你自己部署和运营——没有任何第三方服务，你的笔记不会离开你自己的基础设施。

## 快速上手

1. 部署服务器（单二进制或 Docker Compose，参见[管理员手册](https://github.com/CyberKurry/pkv-sync/blob/main/public-docs/admin-manual.zh-CN.md)），然后打开 `https://your-host/setup` 创建首个管理员账号。
2. 在 Obsidian 社区插件目录中安装并启用 **PKV Sync**。
3. 在插件设置中填入服务器 URL 与部署密钥，登录后选择或创建笔记库。同步自动运行，也提供手动同步命令。

## 隐私

插件只与你配置的服务器 URL 通信，没有任何遥测与统计。服务器管理员（你自己）可以读取存储在服务器上的笔记内容；详见[安全政策](https://github.com/CyberKurry/pkv-sync/blob/main/SECURITY.zh-CN.md)与[部署加固指南](https://github.com/CyberKurry/pkv-sync/blob/main/public-docs/deployment-hardening.zh-CN.md)。

## 文档与支持

- [用户手册](https://github.com/CyberKurry/pkv-sync/blob/main/public-docs/user-manual.zh-CN.md)（另有 English、繁體中文、日本語、한국어 版本）
- [管理员手册](https://github.com/CyberKurry/pkv-sync/blob/main/public-docs/admin-manual.zh-CN.md)
- [安全政策](https://github.com/CyberKurry/pkv-sync/blob/main/SECURITY.zh-CN.md)
- 问题反馈与功能建议：在[本仓库 issues](https://github.com/CyberKurry/pkv-sync-plugin/issues)或[主仓库 issues](https://github.com/CyberKurry/pkv-sync/issues)提交均可

## 许可证

[AGPL-3.0](LICENSE)
