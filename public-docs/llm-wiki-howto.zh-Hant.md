# 使用 PKV Sync 的 LLM Wiki 工作流程

[English](./llm-wiki-howto.md) | [简体中文](./llm-wiki-howto.zh-CN.md) | 繁體中文 | [日本語](./llm-wiki-howto.ja.md) | [한국어](./llm-wiki-howto.ko.md)

文件版本：v1.6.1。

PKV Sync 為由 LLM 維護的 wiki 提供儲存、歷史與 MCP substrate。你自己的 MCP-capable agent 負責執行 LLM，透過普通 PKV Sync 裝置 token 讀寫，並把每個接受的變更提交到筆記庫的 git 歷史。

## 三個層次

使用小而明確的結構，讓人類與 agent 都能理解筆記庫。

- **Sources**：原始筆記、貼上的研究資料、匯入檔案、會議逐字稿，以及其他證據。盡量貼近原始素材保存，並包含足夠來源資訊，以便日後稽核。
- **Wiki**：精簡頁面，用來說明持久的事實、決策、概念、人物、專案或流程。這些頁面彼此連結，並引用來源頁面。
- **Schema**：少量慣例，讓 wiki 可以被 lint，例如必填 frontmatter、索引頁與維護日誌。

PKV Sync 是 substrate，而不是 LLM host。服務端暴露安全的讀取工具、樂觀寫入工具、連結檢查與變更檢查；你選擇的 agent 則決定要摘要、重寫哪些內容，或何時請你確認。

## 連接 agent

建立或重用 PKV Sync 裝置 token，然後用 stdio 將 MCP-capable agent 指向單一筆記庫：

```bash
PKV_TOKEN=pks_xxx pkvsyncd -c /etc/pkv-sync/config.toml mcp --vault <vault-id>
```

對於支援 Streamable HTTP 的 agent，你可以讓它指向 `pkvsyncd serve` 內嵌提供的 `/mcp` 端點，並在每個請求上同時發送部署金鑰與 bearer token。Transport 詳情請參閱 MCP access guide。

給 agent 一段狹窄的指令：讀取 source 頁面、提出 wiki 更新、寫入時使用上次讀取得到的 `parent_commit`，並在事實不確定或出現衝突時停止，等待人工審查。

## 建議 schema

從這個版面配置開始，只有當它對你的工作流程來說太小時才調整：

```text
index.md
log.md
sources/
wiki/
```

使用 `index.md` 作為 wiki 地圖：

```markdown
# Index

## Projects

- [[wiki/project-alpha]]

## Concepts

- [[wiki/sync-model]]
```

使用 `log.md` 作為維護日誌：

```markdown
# Wiki log

## 2026-06-08

- Ingested sources from `sources/meeting-2026-06-08.md`.
- Updated [[wiki/project-alpha]] and checked broken links.
```

在 wiki 頁面使用 frontmatter 保留來源脈絡：

```markdown
---
kind: wiki
sources:
  - sources/meeting-2026-06-08.md
  - sources/spec-phase-1.md
source_commit: 6f1c2ab0d4e5f6789012345678901234567890ab
updated: 2026-06-08
---

# Project Alpha
```

`source_commit` 是該頁面上次調和時所依據的 commit——每次完成一頁的調和就寫上它，筆記庫之後就能告訴你這一頁的證據何時移動了（見下文 [Staleness](#staleness)）。宣告了 `sources:` 卻沒有它的頁面會被回報為未錨定，而不是被拿去和猜測比較。

Source 頁面可以保持原始狀態，但應該標明資訊來源：

```markdown
---
kind: source
origin: "Team meeting"
captured: 2026-06-08
---
```

## Agent 循環

1. Ingest：在 `sources/` 下新增或更新 source 材料，盡量保留原始措辭。當一個 source 會展開成 10 到 25 個 source 與 wiki 頁面時，使用 `write_files`，讓整個 ingest 以一個原子 commit 落地。
2. Query：請 agent 讀取相關 source 與 wiki 頁面，然後提出 `wiki/` 下的更新。
3. Write：只有在 agent 擁有目前的 `parent_commit` 之後，才允許它使用 `write_file`、`write_files`、`move_file` 或 `delete_file`。頁面合併、拆分和歸檔移動時使用 `move_file`，讓 git 能回報重新命名，而不是遺失歷史。大改寫時，先要求 `dry_run: true` 並審查 `would_write` 計畫；把讀取時得到的 `sha256` 值作為 `expected_hashes` 傳入，這樣期間被人改過的頁面會被拒絕（`content_diverged`），而不是被覆寫。
4. Lint：執行 `stale_pages` 取得來源已移動的頁面清單（見下文），執行 `link_graph` 找出孤立、缺失或 ambiguous 連結，並從上次審查過的 commit 執行 `changes_since`，摘要變更內容。
5. Review：檢查提出的 commits、解決衝突，並將不確定的主張保留在 sources 中，直到人類將它們提升到 wiki 頁面。

在 v1.2.1 中，這個循環更適合大型 wiki 筆記庫：批次 ingest 繼續透過 `write_files` 保持原子性，結構性的頁面移動透過 `move_file` 保留歷史，連結和變更工具保持有界並隱藏被過濾路徑，重複同步週期會盡可能重用篩選器、token 檢查和掃描結果快取。

## Staleness

這個工作流程容易陷入的失敗模式是「不可見的腐化」：某個來源改變了，而建立在它之上的頁面仍持續斷言舊的論斷，筆記庫裡卻沒有任何東西說明這件事。只追加的習慣會讓情況更糟——只會不斷增長的知識庫會悄悄累積過時的論斷。

`stale_pages` 回答的是這個問題中機械的那一半。給它 vault id，它會讀取每一頁的 `sources:`／`source_commit:`，把該錨點與目前的 head 做 diff，並回傳證據已經移動的頁面：

- `changed_sources`：來源的內容改變了；
- `removed_sources`：來源已經不存在（引用已移動檔案的頁面也會落在這裡）；
- `renamed`：來源被移動時給出 `{from, to}`，因為頁面引用的仍是舊路徑；
- `unresolved_sources`：服務端無法核實的引用來源——隱藏、被排除或從未存在。`sources:` 裡的拼寫錯誤會出現在這裡，而不是躲在乾淨的結果後面；
- `missing_source_commit`：宣告了來源、但還沒有錨點的頁面；
- `verified: false`：diff 觸及了列表上限，因此該頁的變更可能比顯示的更多。請把未核實的頁面當作需要查看，而不是乾淨。

調和（reconcile），而不是追加：重寫頁面，讓當前最佳答案成為讀者最先讀到的內容；當較新的來源取代較舊的論斷時，要明確說出來（保留舊論斷並指向取代它的內容，而不是靜默刪除它）。然後把該頁的 `source_commit` 更新為你這次調和所依據的 head，並在 `log.md` 記下這次巡檢。

## Lint 例行流程

每次維護完成後，請 agent：

- 使用 vault id 呼叫 `stale_pages`，逐頁調和它回報的每一頁（包括 `unresolved_sources` 裡的拼寫錯誤，以及需要首次錨定的 `missing_source_commit` 頁面），並為它完成的每一頁寫回 `source_commit`；
- 使用 vault id 呼叫 `link_graph`，並回報斷裂連結、ambiguous basename links，以及新的孤立頁面；加上 `kind: "wiki"` 可以只 lint wiki 這一層；
- 使用上次人工審查過的 commit 呼叫 `changes_since`，並摘要新增、修改、刪除與重新命名的頁面；結果帶 `truncated: true` 表示清單被裁切，因此要以更窄的範圍重跑，再下「沒有其它變化」的結論；
- 新增持久 wiki 頁面時，更新 `index.md`；
- 在 `log.md` 附加一則短紀錄，描述來源材料、變更的 wiki 頁面，以及未解問題。

Hidden paths 在整個工作流程中都會保持隱藏。如果某個路徑被 SyncPathFilter 或 exclude glob 拒絕，MCP 讀取工具不會在檔案列表、搜尋結果、連結圖或變更摘要中回報它。
