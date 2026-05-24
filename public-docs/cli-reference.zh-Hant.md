# CLI 參考

[English](./cli-reference.md) | [简体中文](./cli-reference.zh-CN.md) | 繁體中文 | [日本語](./cli-reference.ja.md) | [한국어](./cli-reference.ko.md)

## pkvsyncd materialize

將 PKV Sync vault 的 bare git repository 展開為磁碟上的普通檔案樹。

### 用法

```text
pkvsyncd materialize <vault-id> -o <輸出目錄> [--at <commit-sha>]
```

### 選項

- `-o, --output <DIR>`：輸出目錄，必須不存在或為空。
- `--at <SHA>`：還原到指定 commit，預設為 HEAD。

### 說明

命令會讀取 vault 的 bare git repository，並將每個檔案寫入輸出目錄：

- 文字檔會原樣寫入。
- 以 `pkvsync_pointer` JSON 儲存的二進位檔會從 server blob storage 複製實際 blob。

此命令同步執行，不需要 server 正在運行。它直接讀取設定的 `data_dir` 下的 git repository 與 blob storage。

### 範例

```bash
# 還原最新版本
pkvsyncd materialize abc123 -o ./my-vault

# 還原指定 commit
pkvsyncd materialize abc123 -o ./my-vault-old --at def456
```

### 結束碼

- `0`：成功。
- `1`：錯誤，例如輸出目錄非空、vault 不存在、blob 缺失或 commit SHA 無效。

## pkvsyncd mcp

啟動供 AI 工具使用的 MCP server。

### 用法

```text
pkvsyncd mcp [--transport stdio|http] [--vault <vault-id>] [--token <pks-token>] [--bind <addr>]
```

### 選項

- `--transport <stdio|http>`：transport 模式，預設為 `stdio`。
- `--vault <vault-id>`：stdio 模式必填，只向 client 暴露單一 vault。
- `--token <pks-token>`：stdio 使用的 bearer 裝置 token；省略時讀取 `PKV_TOKEN`。
- `--bind <addr>`：HTTP 監聽地址，預設為 `127.0.0.1:6711`。

### 說明

stdio 模式從 stdin 讀取 JSON-RPC，並向 stdout 寫入 JSON-RPC。HTTP 模式在 `/mcp` 提供無狀態 Streamable HTTP MCP endpoint。兩種模式都暴露 `list_vaults`、`list_files`、`read_file`、`read_file_at_commit`、`search`、`write_file` 和 `delete_file`。

### 範例

```bash
# stdio，token 來自環境變數
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault abc123

# 本機 Streamable HTTP endpoint
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

HTTP 模式每個 request 都需要 server deployment key header。

## pkvsyncd upgrade

將 PKV Sync release binary 下載到目前可執行檔旁邊。

### 用法

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <version>]
```

### 選項

- `--dry-run`：只顯示選中的 release、asset 和目標路徑，不下載。
- `--yes`：跳過互動確認。
- `--version <version>`：下載指定 release，例如 `1.0.0`；省略時下載最新 release。

### 說明

命令會為目前平台選擇 release asset，使用 `SHA256SUMS` 驗證下載內容，將 `pkvsyncd.new` 寫在目前 binary 旁邊（Windows 為 `pkvsyncd.new.exe`），並列印 systemd／手動替換步驟。它不會熱替換正在運行的 server。

Docker 和 Kubernetes 部署應透過拉取或修改 image tag 來升級，然後重啟服務或 rollout。命令偵測到 container 環境時，只會列印 image 升級指引並退出，不會寫入旁路 binary。

### 範例

```bash
# 預覽升級計畫
pkvsyncd upgrade --dry-run

# 下載最新且通過驗證的 binary
pkvsyncd upgrade --yes

# 下載指定 release
pkvsyncd upgrade --yes --version 1.0.0
```
