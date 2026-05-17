# Using git-crypt with PKV Sync

> **Note:** This is a stop-gap guide for end-to-end encryption (E2EE) before native E2EE ships in M3. PKV server can still see filenames and commit metadata.

## Overview

[git-crypt](https://github.com/AGWA/git-crypt) enables transparent file encryption inside a Git repository. Since PKV Sync exposes vaults as Git repositories, you can use git-crypt to encrypt sensitive files before they reach the server.

## Setup

### 1. Install git-crypt

```bash
# macOS
brew install git-crypt

# Ubuntu/Debian
sudo apt install git-crypt

# Windows (via scoop)
scoop install git-crypt
```

### 2. Initialize git-crypt in a cloned vault

```bash
git clone https://_:<token>@your-server/git/<vault-id>
cd <vault-id>
git-crypt init
```

### 3. Configure which files to encrypt

Create or edit `.gitattributes`:

```
# Encrypt all files by default
* filter=git-crypt diff=git-crypt

# But don't encrypt the .gitattributes file itself
.gitattributes !filter !diff
```

For selective encryption (recommended):

```
# Only encrypt specific patterns
secrets/** filter=git-crypt diff=git-crypt
*.key filter=git-crypt diff=git-crypt
*.pem filter=git-crypt diff=git-crypt
```

### 4. Share the key with collaborators

Export the symmetric key:

```bash
git-crypt export-key ../vault-key
```

Each collaborator imports it:

```bash
git-crypt unlock ../vault-key
```

## Limitations

- **Filenames are NOT encrypted.** The PKV server can see file paths and directory structure.
- **git-crypt operates on the Git client side.** The server stores ciphertext blobs. When you clone without the key, encrypted files appear as opaque binary data.
- **Key management is manual.** If a key is lost, encrypted files cannot be recovered.
- **Only works with the Git clone workflow.** The PKV Sync Obsidian plugin does not understand git-crypt. You must clone the vault and work through Git directly for encrypted files.

## Recommended Workflow

1. Use the Obsidian plugin for day-to-day note-taking (unencrypted files).
2. Use Git clone + git-crypt for sensitive files that need E2EE.
3. Keep the git-crypt key backed up securely.

---

# 在 PKV Sync 中使用 git-crypt

> **注意：** 这是在 M3 原生端到端加密（E2EE）发布之前的过渡方案。PKV 服务器仍然可以看到文件名和提交元数据。

## 概述

[git-crypt](https://github.com/AGWA/git-crypt) 可以在 Git 仓库内实现透明的文件加密。由于 PKV Sync 将仓库以 Git 仓库形式暴露，你可以使用 git-crypt 在敏感文件到达服务器之前进行加密。

## 设置

### 1. 安装 git-crypt

```bash
# macOS
brew install git-crypt

# Ubuntu/Debian
sudo apt install git-crypt

# Windows（通过 scoop）
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

```
# 默认加密所有文件
* filter=git-crypt diff=git-crypt

# 但不要加密 .gitattributes 文件本身
.gitattributes !filter !diff
```

选择性加密（推荐）：

```
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

- **文件名未加密。** PKV 服务器可以看到文件路径和目录结构。
- **git-crypt 在 Git 客户端运行。** 服务器存储的是密文。如果你在没有密钥的情况下克隆，加密文件会显示为不透明的二进制数据。
- **密钥管理是手动的。** 如果密钥丢失，加密文件无法恢复。
- **仅适用于 Git 克隆工作流。** PKV Sync Obsidian 插件不理解 git-crypt。你必须克隆仓库并通过 Git 直接操作加密文件。

## 推荐工作流

1. 使用 Obsidian 插件进行日常笔记记录（未加密文件）。
2. 对于需要端到端加密的敏感文件，使用 Git 克隆 + git-crypt。
3. 安全备份 git-crypt 密钥。
