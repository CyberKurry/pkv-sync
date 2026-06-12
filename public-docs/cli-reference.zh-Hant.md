# CLI 參考

[English](./cli-reference.md) | [简体中文](./cli-reference.zh-CN.md) | 繁體中文 | [日本語](./cli-reference.ja.md) | [한국어](./cli-reference.ko.md)

文件版本：v1.3.0。

`pkvsyncd` 是 PKV Sync 的伺服器常駐程式執行檔，提供 HTTP/WebSocket 同步 API、管理介面、MCP 伺服器，以及一小組維運用的子命令。

## 全域選項

下列旗標適用於所有子命令：

- `-c, --config <PATH>`：TOML 設定檔路徑。預設值：`/etc/pkv-sync/config.toml`。
- `-h, --help`：顯示說明。
- `-V, --version`：印出 CLI 版本。

```bash
pkvsyncd -c /opt/pkv-sync/config.toml serve
```

## 子命令

`pkvsyncd` 提供九個子命令。最常用的維運流程是 `serve`、`genkey`、`migrate up`、`user add`、`backup` 與 `restore`。

## pkvsyncd serve

啟動 HTTP 伺服器。

### 用法

```text
pkvsyncd serve
```

### 說明

執行對外的同步 HTTP 監聽器、管理介面、SSE 串流、Git smart HTTP 路由，以及（設定啟用時的）MCP HTTP endpoint。監聽器會綁定到 `config.toml` 中的 `[server].bind_addr`。請以 systemd 前景程序或容器方式執行。

### 範例

```bash
pkvsyncd -c /etc/pkv-sync/config.toml serve
```

## pkvsyncd migrate

資料庫遷移命令。目前只支援 `up` 一種操作。

### 用法

```text
pkvsyncd migrate up
```

### 說明

對 `[storage].db_path` 所指的資料庫，套用 `server/migrations/` 目錄下所有尚未執行的 SQLite 遷移。可重複執行，已套用的遷移會被略過。HTTP 伺服器啟動時也會自動執行待套用的遷移，因此手動執行 `migrate up` 通常只在冷還原流程或為離線備份做遷移時才需要。

### 範例

```bash
pkvsyncd migrate up
```

## pkvsyncd genkey

產生一組可用於 `[server].deployment_key` 的隨機部署金鑰。

### 用法

```text
pkvsyncd genkey
```

### 說明

向 stdout 印出一組以密碼學亂數產生的 `k_*` token。請將該值貼到 `config.toml`，並透過你自有的安全通道分發給外掛／管理端的客戶端。

### 範例

```bash
pkvsyncd genkey
# k_3f4a5e6b7c8d9e0f1a2b3c4d5e6f7a8b
```

## pkvsyncd user

使用者管理命令。適合用於維運層級的復原（忘記密碼、帳號被停用）以及以指令稿啟動次要操作員帳號。

### 用法

```text
pkvsyncd user add <USERNAME> [--admin]
pkvsyncd user passwd <USERNAME>
pkvsyncd user list
pkvsyncd user set-active <USERNAME> --active <true|false>
```

### 子命令

- `add <USERNAME> [--admin]`：建立使用者，並以互動方式提示輸入密碼。
- `passwd <USERNAME>`：重設使用者密碼，並提示輸入新值。
- `list`：列出所有使用者，包含其管理員／啟用狀態與建立時間。
- `set-active <USERNAME> --active <true|false>`：停用或重新啟用使用者。被停用的使用者仍保有自身的 token，但無法登入或同步。

### 範例

```bash
# 為緊急存取建立管理員帳號
pkvsyncd user add alice --admin

# 重設忘記的密碼
pkvsyncd user passwd alice

# 停用離職使用者但不刪除其資料
pkvsyncd user set-active alice --active false
```

## pkvsyncd materialize

將 PKV Sync vault 的 bare git repository 展開為磁碟上的普通檔案樹。

### 用法

```text
pkvsyncd materialize <VAULT-ID> -o <OUTPUT-DIR> [--at <COMMIT-SHA>]
```

### 選項

- `-o, --output <DIR>`：輸出目錄，必須不存在或為空。
- `--at <SHA>`：還原到指定 commit，預設為 HEAD。

### 說明

讀取 vault 在 `data_dir/vaults/<vault-id>` 下的 bare git repository，並將每個檔案寫入輸出目錄：

- 文字檔會原樣寫入。
- 以 `pkvsync_pointer` JSON 儲存的二進位檔，會從伺服器的 blob 儲存區（`data_dir/blobs/`）複製實際的 blob。

此命令同步執行，不需要伺服器正在運行。它直接從設定的 `data_dir` 下的磁碟 git repository 與 blob 儲存區讀取資料。

### 範例

```bash
# 還原最新版本
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault

# 還原指定 commit
pkvsyncd materialize 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c -o ./my-vault-old --at abc123def456
```

### 結束碼

- `0`：成功。
- `1`：錯誤，例如輸出目錄非空、找不到 vault、blob 缺失或 commit SHA 無效。

> Vault ID 為 32 個字元的小寫十六進位字串（不含破折號）。上述範例皆採用真實格式的 ID；管理介面與 `pkvsyncd user list` 也會顯示有效的 ID。

## pkvsyncd backup

將伺服器資料快照成可攜的備份目錄。

### 用法

```text
pkvsyncd backup -o <OUTPUT-DIR> [--data-dir <DIR>] [--gzip] [--include-config]
```

### 選項

- `-o, --output <DIR>`：備份輸出目錄，必須不存在或為空。
- `--data-dir <DIR>`：離線操作時用以覆寫的資料目錄。預設為已載入設定中的 `[storage].data_dir`。
- `--gzip`：在備份目錄旁額外建立一份 `.tar.gz` 壓縮檔。
- `--include-config`：把已載入的 `config.toml` 一併寫入備份。預設備份會省略設定檔，因為其中可能包含部署金鑰和本機秘密。

### 說明

將 SQLite 資料庫（透過 VACUUM INTO 進行，因此不會阻塞來源）、每個 vault 的 bare git repository，以及 blob 儲存區，快照到一個獨立目錄，並寫入 `MANIFEST.json`。備份期間 HTTP 伺服器可繼續運行；複製各個 vault 的 repository 時，會暫時、逐一靜止對應 vault 的推送。

預設情況下，備份會省略 `config.toml`；只有在你明確要保存設定並保護其中秘密時，才加入 `--include-config`。

### 範例

```bash
pkvsyncd backup -o /var/backups/pkv-2026-05-25 --gzip
```

## pkvsyncd restore

將備份目錄還原到資料目錄中。

### 用法

```text
pkvsyncd restore -i <BACKUP-DIR> [--data-dir <DIR>] [--force]
```

### 選項

- `-i, --input <DIR>`：包含 `MANIFEST.json` 的備份目錄。
- `--data-dir <DIR>`：用以覆寫的目標資料目錄。預設為 `[storage].data_dir`。
- `--force`：在還原前清空非空的目標資料目錄。

### 說明

驗證備份的 `MANIFEST.json`，將 SQLite 資料庫、各 vault repository 與 blob 儲存區複製到目標資料目錄。還原前請先停止 HTTP 伺服器。若還原的備份是由較舊版本的伺服器所產生，還原完成後請再執行一次 `pkvsyncd migrate up`。

### 範例

```bash
pkvsyncd restore -i /var/backups/pkv-2026-05-25 --data-dir /var/lib/pkv-sync --force
```

## pkvsyncd verify

驗證各 vault 的 git repository 與內容定址的 blob。

### 用法

```text
pkvsyncd verify [--data-dir <DIR>] [--no-fail]
```

### 選項

- `--data-dir <DIR>`：用以覆寫的資料目錄。
- `--no-fail`：即使驗證發現錯誤，仍回傳結束碼 0。適合僅需記錄而不希望觸發告警的監控指令稿。

### 說明

對 `data_dir/vaults/` 之下的每個 vault：

- 對 bare repository 執行 `git fsck --strict`。
- 走訪 HEAD 樹，並驗證每個 `pkvsync_pointer` 都可解析到對應 blob，且該 blob 在磁碟上的 SHA-256 與其檔名一致。

按 vault 逐一回報錯誤數量。只要任一 vault 有錯誤，便以非零結束碼結束；除非加上 `--no-fail`。

### 範例

```bash
pkvsyncd verify --data-dir /var/lib/pkv-sync
```

## pkvsyncd mcp

啟動供 AI 工具使用的 MCP（Model Context Protocol）伺服器。

### 用法

```text
pkvsyncd mcp [--transport stdio|http] [--vault <VAULT-ID>] [--token <PKS-TOKEN>] [--bind <ADDR>]
```

### 選項

- `--transport <stdio|http>`：傳輸模式。預設為 `stdio`。
- `--vault <VAULT-ID>`：stdio 模式必填，指定要向客戶端暴露的單一 vault。
- `--token <PKS-TOKEN>`：stdio 使用的 bearer 裝置 token。省略時會改用 `PKV_TOKEN` 環境變數。
- `--bind <ADDR>`：HTTP 監聽位址。預設為 `127.0.0.1:6711`。

### 說明

`stdio` 模式從 stdin 讀取 JSON-RPC，並向 stdout 寫入 JSON-RPC。`http` 模式在 `/mcp` 提供無狀態的 Streamable HTTP MCP endpoint。兩種模式皆暴露同一組工具：`list_vaults`、`list_files`、`read_file`、`read_file_at_commit`、`search`、`link_graph`、`changes_since`、`write_file`、`delete_file`、`write_files` 與 `move_file`。`write_files` 適合原子的多頁 wiki 編輯，`move_file` 適合保留歷史的重新命名或歸檔移動。寫入類工具有速率限制，每組 `(token, vault)` 每分鐘上限 60 次寫入，且一個 `write_files` 批次只消耗一次寫入記錄。搜尋請求最多掃描 5000 個可見 tree 檔案、返回 500 條匹配，並在生產環境搜尋文字累計達到 256 MiB 後停止。`link_graph` 最多掃描 5000 個可見文字檔，並使用同一個生產文字預算；`changes_since` 最多返回 5000 條可見變更。超過 64 MiB 的二進位/blob 讀取回應會被拒絕，而不是被 base64 展開進 JSON。

`http` 模式要求每個 request 都必須帶上伺服器部署金鑰的 header，與一般同步 API 相同。


這個子命令仍然是獨立 MCP 進程。若要把同一個 Streamable HTTP transport 掛到主服務端口，請設定 `[mcp].embed_in_serve = true` 並執行 `pkvsyncd serve`。
### 範例

```bash
# stdio，token 來自環境變數
PKV_TOKEN=pks_xxx pkvsyncd mcp --vault 6c0a2b8f4d3e419a8c5b7f1d2e3a4b5c

# 本機 Streamable HTTP endpoint
pkvsyncd mcp --transport http --bind 127.0.0.1:6711
```

## pkvsyncd upgrade

將 PKV Sync 的 release binary 下載到目前可執行檔旁邊。

### 用法

```text
pkvsyncd upgrade [--dry-run] [--yes] [--version <VERSION>]
```

### 選項

- `--dry-run`：只顯示選中的 release、asset 與目標路徑，不實際下載。
- `--yes`：略過互動確認提示。
- `--version <VERSION>`：下載指定 release，例如 `1.3.0`，而非最新版本。

### 說明

此命令會為目前平台挑選對應的 release asset，依 `SHA256SUMS` 驗證下載內容，將 `pkvsyncd.new` 寫入目前 binary 的旁邊（Windows 為 `pkvsyncd.new.exe`），並印出 systemd 或手動切換的步驟。它不會熱替換正在運行中的伺服器。

Docker 與 Kubernetes 部署應改以拉取或更換 image tag 的方式升級，並重啟服務或進行 rollout。當命令偵測到容器環境時，只會印出以 image 為主的升級指引並退出，不會寫入旁路 binary。

### 範例

```bash
# 預覽升級計畫
pkvsyncd upgrade --dry-run

# 下載最新且通過驗證的 binary
pkvsyncd upgrade --yes

# 下載指定 release
pkvsyncd upgrade --yes --version 1.3.0
```
