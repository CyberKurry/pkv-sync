# 在 PKV Sync 中使用 git-crypt

[English](./git-crypt-howto.md) | [简体中文](./git-crypt-howto.zh-CN.md) | 繁體中文 | [日本語](./git-crypt-howto.ja.md) | [한국어](./git-crypt-howto.ko.md)

> **注意：** 這是原生端到端加密（E2EE）發布前的過渡方案。PKV Sync server 仍然可以看到檔名和 commit metadata。

## 概述

[git-crypt](https://github.com/AGWA/git-crypt) 可在 Git repository 內實現透明檔案加密。由於 PKV Sync 將 vault 暴露為 Git repository，你可以在敏感檔案到達 server 前使用 git-crypt 加密。

## 設定

### 1. 安裝 git-crypt

```bash
# macOS
brew install git-crypt

# Ubuntu/Debian
sudo apt install git-crypt

# Windows, via scoop
scoop install git-crypt
```

### 2. 在 clone 的 vault 中初始化 git-crypt

```bash
git clone https://_:<token>@your-server/git/<vault-id>
cd <vault-id>
git-crypt init
```

### 3. 設定要加密的檔案

建立或編輯 `.gitattributes`：

```gitattributes
# 預設加密所有檔案
* filter=git-crypt diff=git-crypt

# 但不要加密 .gitattributes 本身
.gitattributes !filter !diff
```

建議使用選擇性加密：

```gitattributes
# 只加密特定 patterns
secrets/** filter=git-crypt diff=git-crypt
*.key filter=git-crypt diff=git-crypt
*.pem filter=git-crypt diff=git-crypt
```

### 4. 與協作者分享 key

匯出 symmetric key：

```bash
git-crypt export-key ../vault-key
```

每位協作者匯入：

```bash
git-crypt unlock ../vault-key
```

## 限制

- **檔名不會加密。** PKV Sync server 可以看到檔案路徑和目錄結構。
- **git-crypt 在 Git client 端運作。** Server 儲存的是 ciphertext blobs。沒有 key 時 clone，encrypted files 會顯示為不透明 binary data。
- **Key management 是手動的。** Key 遺失時 encrypted files 無法復原。
- **只適用於 Git clone workflow。** PKV Sync Obsidian 外掛不了解 git-crypt。你必須 clone vault 並透過 Git 直接處理 encrypted files。
- **`pkvsyncd materialize` 不認識 git-crypt。** 被 PKV Sync 以 `pkvsync_pointer` JSON 儲存的檔案（通常是不在文字副檔名清單內的二進位檔），會在 materialize 時對照 server 的 blob store 還原為原始 bytes —— 用戶端的 git-crypt filter 完全看不到這些檔案，因此用 git-crypt 加密 `*.pdf` 或其他被存為 blob 的副檔名，不會產生預期的密文流。請把 git-crypt patterns 限制在 PKV Sync 視為文字的檔案類型（伺服器設定的 `text_extensions` 清單，預設為 `md`、`canvas`、`base`、`json`、`txt`、`css`）。

## 建議工作流

1. 日常筆記使用 Obsidian 外掛處理未加密檔案。
2. 需要 E2EE 的敏感檔案使用 Git clone 和 git-crypt。
3. 安全備份 git-crypt key。
