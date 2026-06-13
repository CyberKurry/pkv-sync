# AI 工具的 MCP 接入

[English](./mcp-howto.md) | [简体中文](./mcp-howto.zh-CN.md) | 繁體中文 | [日本語](./mcp-howto.ja.md) | [한국어](./mcp-howto.ko.md)

文件版本：v1.4.0。

PKV Sync 可以透過 MCP server 暴露筆記庫內容。服務端返回檔案內容前會解析 blob pointer，也可以透過顯式讀寫工具寫入檔案，並且必須使用普通 PKV Sync bearer 裝置 token。

## 工具

- `list_vaults`：列出目前使用者可存取的筆記庫。
- `list_files {vault_id, at?}`：列出 HEAD 下的路徑；設定 `at` 時則列出該 commit SHA 下的路徑。
- `read_file {vault_id, path}`：讀取 HEAD 下的檔案。
- `read_file_at_commit {vault_id, path, commit}`：讀取指定 commit 下的檔案。
- `search {vault_id, query, at?, limit?}`：在文字檔案中執行大小寫不敏感的子字串搜尋。`at` 將範圍限定到歷史 commit；`limit` 限制回傳的命中數量。
- `link_graph {vault_id, at?, path_prefix?, limit?}`：返回筆記庫的 wikilink 與 Markdown 連結圖。回應包含每個檔案的節點及其 `outlinks` 與計算出的 `inlinks`、孤立頁面、帶有 `missing` 或 `ambiguous` 原因的斷裂連結，以及 `truncated` 標記。
- `changes_since {vault_id, since_commit, path_prefix?, limit?}`：列出自 `since_commit` 以來新增、修改、刪除或重新命名的檔案。回應包含 `from_commit`、目前的 `to_commit`、`changes` 與 `truncated`；如果 `since_commit` 不是 HEAD 的祖先，工具會返回 `unrelated_commit`，讓用戶端重新讀取筆記庫。
- `write_file {vault_id, path, content, parent_commit}`：以 `parent_commit` 樂觀並發控制建立或更新文字檔案。
- `delete_file {vault_id, path, parent_commit}`：以 `parent_commit` 樂觀並發控制刪除檔案。
- `write_files {vault_id, parent_commit, writes?, deletes?}`：在一個 commit 中原子地建立、更新和／或刪除多個文字檔案。`writes[]` 包含 `{path, content}` 物件；`deletes[]` 包含路徑。
- `move_file {vault_id, parent_commit, from, to}`：在一個 commit 中移動或重新命名文字檔案，並保留 git rename 歷史。目標路徑不能已經存在。

所有 MCP 讀取工具都遵守目前的 SyncPathFilter。被內建隱藏路徑規則或執行階段 exclude globs 拒絕的路徑，不會被列出、搜尋、讀取、納入連結圖，或回報為變更。

## stdio transport

本機 AI 工具需要啟動命令時，使用 stdio。stdio 模式只暴露一個筆記庫。

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

也可以直接傳入 token：

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

## Streamable HTTP transport

當用戶端連接一個已經執行的本機或內部 MCP 端點時，使用 HTTP。PKV Sync 提供兩種 HTTP 部署模式：

- **內嵌模式**：在 `config.toml` 中設定 `[mcp].embed_in_serve = true`，`pkvsyncd serve` 會在主服務端口掛載 `/mcp`。
- **獨立模式**：執行單獨的 MCP 進程，適合專用監聽位址、隔離 MCP 或獨立擴縮容：

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --transport http --bind 127.0.0.1:6711
```

端點路徑始終是 `/mcp`；內嵌模式使用主服務 origin，獨立模式使用單獨的監聽位址：

```text
POST http://127.0.0.1:6711/mcp
GET  http://127.0.0.1:6711/mcp
```

每個請求都必須包含：

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

部署金鑰來自與主 PKV Sync 服務相同的設定檔。缺少或錯誤的部署金鑰會在 bearer token 驗證前直接回傳 HTTP `404`。

MCP HTTP 使用固定視窗限流，每 60 秒最多 120 次請求。超限時，伺服器會返回 HTTP `429`，JSON-RPC error code 為 `-32029`。失敗的 MCP bearer token 認證也會在進程內限流，stdio 和 HTTP transport 合計每 60 秒最多 30 次失敗嘗試。

POST 承載 JSON-RPC 工具呼叫並返回 JSON 回應。GET 攜帶 `Accept: text/event-stream` 時訂閱 `vault_changed` notification。事件 id 使用 `<vault-id>:<commit-sha>`，用戶端重連時可作為 `Last-Event-ID` 傳回，以 replay 斷線期間錯過的 commit。Replay 有上限；如果服務端無法覆蓋錯過的歷史，會發送 `lagged`，用戶端應透過同步 API 重新整理。

除非放在可信網路控制之後，否則請把 HTTP 綁定到 loopback。bearer token 會授予該使用者所有筆記庫的讀寫存取權限。

## 讀取和搜尋上限

`search` 最多掃描 5000 個可見 tree 檔案，最多返回 500 條匹配，並在生產環境搜尋文字累計達到 256 MiB 後停止。`link_graph` 最多掃描 5000 個可見文字檔案，並使用相同的生產環境文字預算。`changes_since` 最多返回 5000 條可見變更項目。`read_file` 和 `read_file_at_commit` 會在返回前解析 blob pointer；超過 64 MiB 的二進位/blob 回應會被拒絕，而不是被 base64 展開進 JSON。

## 寫入工具

PKV Sync 在讀取工具之外提供四個 MCP 寫入工具：

- `write_file(vault_id, path, content, parent_commit)`：建立或更新文字檔案。
- `delete_file(vault_id, path, parent_commit)`：刪除檔案。
- `write_files(vault_id, parent_commit, writes[], deletes[])`：在一個 commit 中原子地建立、更新和刪除多個文字檔案。如果任一路徑無效、檔案超過 `max_file_size`、批次為空（`empty_batch`），或批次超過 100 個變更（`batch_too_large`），服務端不會提交任何內容。陳舊的 `parent_commit` 會返回常規 `Conflict` 回應。
- `move_file(vault_id, parent_commit, from, to)`：在單個 commit 中移動或重新命名一個文字檔案。它會拒絕已存在的目標（`target_exists`）、二進位／blob-pointer 來源檔案（`unsupported_binary_move`），以及缺失或隱藏的來源檔案（`not_found`）。

### 樂觀並發控制

每次寫入都必須提供 `parent_commit`，也就是用戶端認為目前筆記庫 HEAD 所在的 commit hash。如果用戶端上次讀取後筆記庫已經前進，服務端會返回 `{ "conflict": true, "current_head": "..." }`，並且不會寫入。用戶端需要重新讀取、必要時合併，再用新的 `parent_commit` 重試。

### 限流

寫入工具按 `(token, vault)` 組合限流，每分鐘最多 60 次寫入。`write_files` 整個批次只消耗一次限流記錄。讀取工具和 SSE 訂閱不受這個寫入配額影響。

1.2.1 的加固讓寫入驗證保持 fail-closed：`writes[]` 和 `deletes[]` 中正規化後重複的路徑會被拒絕，隱藏或排除路徑不會洩漏目標存在性，無效的 `move_file` 來源會在消耗寫入配額前被拒絕。MCP 驗證錯誤保持泛化，Streamable HTTP JSON request body 上限為 100 MiB。

### 稽核記錄

每次成功寫入、批量寫入、移動或刪除都會在活動日誌中記錄為 `mcp_write` 或 `mcp_delete`，details 中包含路徑摘要、commit 和 size。管理員可以在活動頁查看 AI 驅動的改動。

### 注意：寫入會進入 git 歷史

AI 驅動的寫入會成為筆記庫 git 歷史中的 commit。你可以透過普通 git 操作回滾，但無法讓已經提交的改動「從未發生」；這種可稽核性是有意設計。

## 用戶端提示

- Claude Code、Codex CLI、Cherry Studio、OpenCode，以及透過橋接使用 MCP 的用戶端，都可以透過啟動 `pkvsyncd mcp` 使用 stdio 模式。
- 支援 Streamable HTTP 的用戶端可以指向 `/mcp`，並在每個請求上發送 bearer auth 與部署金鑰。
- 服務端是無狀態的，不要求也不返回 `Mcp-Session-Id`。
