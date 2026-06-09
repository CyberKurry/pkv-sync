# 在 PKV Sync 中使用 git-crypt

[English](./git-crypt-howto.md) | 简体中文 | [繁體中文](./git-crypt-howto.zh-Hant.md) | [日本語](./git-crypt-howto.ja.md) | [한국어](./git-crypt-howto.ko.md)

文档版本：v1.2.0。

> **注意：** 这是原生端到端加密（E2EE）发布前的过渡方案。PKV Sync 服务器仍然可以看到文件名和提交元数据。

## 概述

[git-crypt](https://github.com/AGWA/git-crypt) 可以在 Git 仓库内实现透明的文件加密。由于 PKV Sync 将仓库以 Git 仓库形式暴露，你可以使用 git-crypt 在敏感文件到达服务器之前进行加密。

## 设置

### 1. 安装 git-crypt

```bash
# macOS
brew install git-crypt

# Ubuntu/Debian
sudo apt install git-crypt

# Windows，通过 scoop
scoop install git-crypt
```

### 2. 在克隆的仓库中初始化 git-crypt

```bash
git clone https://_:<token>@your-server/git/<vault-id>
cd <vault-id>
git-crypt init
```

### 3. 配置要加密的文件

创建或编辑 `.gitattributes`：

```gitattributes
# 默认加密所有文件
* filter=git-crypt diff=git-crypt

# 但不要加密 .gitattributes 文件本身
.gitattributes !filter !diff
```

选择性加密（推荐）：

```gitattributes
# 只加密特定模式
secrets/** filter=git-crypt diff=git-crypt
*.key filter=git-crypt diff=git-crypt
*.pem filter=git-crypt diff=git-crypt
```

### 4. 与协作者共享密钥

导出对称密钥：

```bash
git-crypt export-key ../vault-key
```

每位协作者导入：

```bash
git-crypt unlock ../vault-key
```

## 限制

- **文件名未加密。** PKV Sync 服务器可以看到文件路径和目录结构。
- **git-crypt 在 Git 客户端运行。** 服务器存储的是密文。如果你在没有密钥的情况下克隆，加密文件会显示为不透明的二进制数据。
- **密钥管理是手动的。** 如果密钥丢失，加密文件无法恢复。
- **仅适用于 Git 克隆工作流。** PKV Sync Obsidian 插件不理解 git-crypt。你必须克隆仓库并通过 Git 直接操作加密文件。
- **`pkvsyncd materialize` 不感知 git-crypt。** PKV Sync 以 `pkvsync_pointer` JSON 形式存储的文件（通常是文本扩展名清单之外的二进制文件）在 materialize 时会从服务器的 blob 存储中解析，并以原始字节落地——git-crypt 的 filter 在客户端根本看不到它们，因此通过 git-crypt 加密 `*.pdf` 或其他以 blob 形式存储的扩展名，不会得到预期的密文流。请把 git-crypt 模式限制在 PKV Sync 视为文本的扩展名（服务器配置的 `text_extensions` 列表，默认为 `md`、`canvas`、`base`、`json`、`txt`、`css`）。

## 推荐工作流

1. 使用 Obsidian 插件进行日常笔记记录（未加密文件）。
2. 对于需要端到端加密的敏感文件，使用 Git 克隆和 git-crypt。
3. 安全备份 git-crypt 密钥。
