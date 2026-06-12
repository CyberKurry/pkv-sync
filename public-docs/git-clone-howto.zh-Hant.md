# 透過 Git clone 你的 PKV vault

[English](./git-clone-howto.md) | [简体中文](./git-clone-howto.zh-CN.md) | 繁體中文 | [日本語](./git-clone-howto.ja.md) | [한국어](./git-clone-howto.ko.md)

文件版本：v1.3.1。

PKV Sync 可以將每個 vault 透過 HTTPS 暴露為唯讀 Git repository。

## 前置條件

- Server admin 已在 Sync & Storage settings 啟用「Git smart HTTP」。
- Server 上有可用的 `git` binary。
- 你擁有有效的 device token。

## Clone

```bash
git clone https://_:<token>@your-server/git/<vault-id>
```

冒號前的底線是 username。可填任意值；只有 password 位置的 token 會被使用。

### 範例

如果 server 是 `sync.example.com`、vault ID 是 `6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c`、device token 是 `pks_0f1e2d3c4b5a6978...`，執行：

```bash
git clone https://_:pks_0f1e2d3c4b5a6978@sync.example.com/git/6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c
```

Vault ID 是 32 個字元的小寫 hex（不含連字號）。Admin WebUI 與 `pkvsyncd user list` 會顯示有效 ID；像 `abc123` 這類佔位字串會被以 `400 invalid_vault_id` 拒絕。

## Materialize

Clone 之後，blob 檔案會顯示為 pointer JSON，因為 PKV Sync server 會單獨儲存大檔案。執行：

```bash
pkvsyncd materialize <vault-id> -o ./output
```

這會將 pointer 檔案替換為實際二進位內容，產生完整可用的本機 vault copy。

## 注意事項

- HTTP 上的 repository 是**唯讀**。你不能透過 Git push 變更。
- 請使用 PKV Sync 外掛進行變更，並透過一般 sync API push。
- 如果 server admin 停用 Git smart HTTP，clone 或 fetch 會回傳 HTTP 503。
