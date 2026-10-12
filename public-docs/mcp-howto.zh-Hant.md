# AI 工具的 MCP 接入

[English](./mcp-howto.md) | [简体中文](./mcp-howto.zh-CN.md) | 繁體中文 | [日本語](./mcp-howto.ja.md) | [한국어](./mcp-howto.ko.md)

文件版本：v1.5.1。

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

被目前 `SyncPathFilter` 拒絕的路徑不會被列出、搜尋、讀取，也不會納入連結圖或變更報告。讀取類工具（`list_files`、`read_file`、`read_file_at_commit`、`search`）會接受被按 vault allowlist 重新納入的隱藏路徑，並可能返回自動生成的 `.conflict-*` 旁檔。`link_graph`、`changes_since` 以及所有寫入類工具採用更嚴格的政策，直接拒絕隱藏路徑，因此 agent 無法對隱藏檔案執行操作。

## stdio transport

本機 AI 工具需要啟動命令時，使用 stdio。stdio 模式只暴露一個筆記庫。

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

也可以直接傳入 token：

```bash
pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id> --token pks_xxx
```

stdio 同時支援兩個協定時代。現代 2026-07-28 用戶端在請求中發送 `_meta` 協定欄位，並收到現代結果封裝；舊版用戶端透過 `initialize` 協商，伺服器會回顯所請求的、它已知的版本（包括 `2026-07-28`），未知版本則回退到最新的舊版本（`2025-11-25`）。現代 stdio 用戶端還可以對限定的筆記庫呼叫 `subscriptions/listen`；工作階段隨後會持續輸出 `notifications/resources/updated` 行，直到用戶端傳送攜帶該 listen 請求 id 的 `notifications/cancelled` 通知（或退出）。

## Streamable HTTP transport

當用戶端連接一個已經執行的本機或內部 MCP 端點時，使用 HTTP。MCP HTTP 僅支援內嵌部署：`[mcp].embed_in_serve` 預設為 `true`，`pkvsyncd serve` 會在主服務端口掛載 `/mcp`。設定 `[mcp].embed_in_serve = false` 可停用內嵌端點。獨立進程 `pkvsyncd mcp --transport http --bind` 已在本版本移除；原有的獨立部署請改為指向主服務的 `/mcp`。

端點路徑始終是 `/mcp`，位於主服務 origin：

```text
POST http://127.0.0.1:6710/mcp
GET  http://127.0.0.1:6710/mcp
```

每個請求都必須包含：

```text
X-PKVSync-Deployment-Key: k_xxx
Authorization: Bearer pks_xxx
```

部署金鑰來自與主 PKV Sync 服務相同的設定檔。缺少或錯誤的部署金鑰會在 bearer token 驗證前直接回傳 HTTP `404`。

MCP HTTP 使用固定視窗限流，每 60 秒最多 120 次請求。超限時，伺服器會返回 HTTP `429`，JSON-RPC error code 為 `-63001`。失敗的 MCP bearer token 認證返回 HTTP `401` 和 code `-63002`，並且同樣按 transport 實例在進程內限流：內嵌的 HTTP 端點共享伺服器進程的限流器，而每個 `pkvsyncd mcp` 進程會自建狀態，因而擁有自己的限流器，每 60 秒最多 30 次失敗嘗試。

MCP 規範中的 `Origin` 請求頭 DNS-rebinding 檢查未在 `/mcp` 上實作，這是有意為之：該端點要求非環境憑證的部署金鑰和 bearer 裝置 token（瀏覽器無法在跨源請求中附帶這兩者），管理員會話 cookie 會被拒絕，並且沒有任何 CORS 政策向瀏覽器源暴露該端點。如果某個部署關閉了這些前提中的任何一項，就應當為 `/mcp` 自行前置 `Origin` 檢查。

## MCP 2026-07-28（現代）用戶端

同一個端點同時服務兩個協定時代。現代用戶端在 `params._meta` 中發送逐請求協定欄位（`io.modelcontextprotocol/protocolVersion` 和 `io.modelcontextprotocol/clientCapabilities`），並且必須在 `MCP-Protocol-Version` 請求頭中重複版本、在 `Mcp-Method` 請求頭中重複方法；當請求攜帶 `params.name` 時，還要在 `Mcp-Name` 請求頭中重複名稱（非純 ASCII 時以 `=?base64?...?=` 形式 base64 編碼）。任一請求頭與請求體不符都會返回 HTTP `400` 和 code `-32020`。請求不支援的版本會返回 HTTP `400` 和 code `-32022`，並附上受支援版本列表（`2026-07-28`）。現代請求中的無效參數（例如 `_meta` 格式錯誤，或工具 `arguments` 不是物件）會返回 HTTP `400` 和 code `-32602`。未知方法返回 HTTP `404` 和 `-32601`；不含 `id` 的純通知訊息以 HTTP `202` 應答且無回應體，但 `notifications/cancelled` 除外——它返回 `405`：在 Streamable HTTP 上，取消是透過關閉 SSE 回應串流完成的。

現代結果帶有 `resultType`，`tools/list` 和 `server/discover` 還會額外帶有 `ttlMs`/`cacheScope` 快取提示。`server/discover` 會報告 `supportedVersions`，用戶端可以在連線前據此選擇協定時代。（僅現代版本；舊版 `initialize` 協商不在此列表內）舊版 `initialize` 用戶端仍可使用：伺服器會回顯所請求的舊版本；若請求的是現代版本 `2026-07-28` 則原樣回顯，未知版本則回退到最新的舊版本（`2025-11-25`），並以舊版封裝應答。

## 變更通知

- **舊版用戶端**：攜帶 `Accept: text/event-stream` 的 GET `/mcp` 訂閱 `vault_changed` 通知。每個事件的 id 就是 commit SHA 本身，用戶端重連時可作為 `Last-Event-ID` 傳回，以 replay 該 vault 錯過的 commit。Replay 有上限；如果服務端無法覆蓋錯過的歷史，會發送 `lagged`，用戶端應透過同步 API 重新整理。包含 `:` 的值不會被當作續傳位置；當權杖可見多個 vault 時，只有正在續傳的那個 vault 位置已知，因此服務端也會為其他 vault 發送 `lagged`。直播產生的 `lagged` 事件在事件 data 中攜帶受影響的 vault id；回放級別的 `lagged` 事件 data 為空。
- **現代用戶端**：POST 一個 `subscriptions/listen` 請求，將 `params.notifications.resourceSubscriptions` 設為 `["pkv://vault/<id>"]` URI（只有權杖可見的 vault 會被接受）。`resourceSubscriptions` 中重複的 URI 會在確認（acknowledgment）中去重（以首次出現的為準）。回應是一條 SSE 串流：先傳送回顯已接受訂閱的 `notifications/subscriptions/acknowledged` 事件，隨後傳送 `notifications/resources/updated` 事件，其 params 攜帶 `uri`、`commit`、`event` 和 `_meta.subscriptionId`。如果伺服器的事件緩衝區溢出（串流消費較慢時一次湧入超過 64 個 commit），listen 串流會傳送 `notifications/resources/dropped` 通知，攜帶受影響的 `uri` 和同一個 `_meta` 訂閱 id；用戶端應當重新讀取該 vault 的狀態。關閉串流即取消訂閱。現代串流不可續傳：沒有 `Last-Event-ID` replay，重連後用戶端需要重新 listen 並比對讀取結果。

除非放在可信網路控制之後，否則請把 HTTP 綁定到 loopback。bearer token 會授予該使用者所有筆記庫的讀寫存取權限。

## 讀取和搜尋上限

`search` 最多掃描 5000 個可見 tree 檔案——當 vault 的可見檔案超過 5000 個時，呼叫會以 `too many files to search` 失敗，而不是只掃描一部分。它最多返回 500 條匹配（預設 100），並在生產環境已讀取文字超過 256 MiB 後報錯中止。`link_graph` 最多掃描 5000 個可見文字檔案，並使用相同的生產環境文字預算，但會返回 `truncated: true` 而不是失敗；單檔連結擷取上限為 10000 條、失效連結累計上限為 20000 條，觸及任一上限都會設定 `truncated: true`。`changes_since` 最多返回 5000 條可見變更項目。`read_file` 和 `read_file_at_commit` 會在返回前解析 blob pointer；二進位內容以 base64 返回並帶有 `encoding: "base64"`，超過 64 MiB 的二進位/blob 回應會被拒絕，而不是被 base64 展開進 JSON。

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
- 兩種 transport 均為雙時代：舊版 `initialize` 用戶端繼續可用，現代 2026-07-28 用戶端可獲得逐請求版本處理、`server/discover` 和 `subscriptions/listen` 推送。
- 服務端是無狀態的，不要求也不返回 `Mcp-Session-Id`。
