# 升級說明：0.x 到 1.0

[English](./upgrade-notes-v1.0.md) | [简体中文](./upgrade-notes-v1.0.zh-CN.md) | 繁體中文 | [日本語](./upgrade-notes-v1.0.ja.md) | [한국어](./upgrade-notes-v1.0.ko.md)

文件版本：v1.3.1。

PKV Sync 1.0 是第一個穩定版。它也為後續 1.x 維護重置了 SQLite migration 基線。

## 重要資料庫說明

PKV Sync 1.0 只發布一個 `0001_initial.sql` 基線 migration。由 0.x 版本建立的 SQLite 資料庫**不支援原地升級**到 1.0.0。

如果你正在執行 0.x 服務端，請選擇下面路徑之一：

1. 舊部署只在遷移準備期間停留在最終 0.8.x patch 版本，用於備份、materialize 或匯出資料。
2. 先備份或 materialize 每個筆記庫，使用全新的 1.0 資料目錄啟動服務，重新建立使用者和筆記庫，然後把筆記庫內容匯入或 push 到新服務端。
3. 在任何遷移演練前，先用 `pkvsyncd backup` 保存完整的 0.x 資料根目錄。

不要把 1.0 二進位或 Docker 映像直接指向既有的 0.x `metadata.db`。

## 1.0 穩定承諾

從 1.0 開始，以下表面遵循語義化版本：

- `public-docs/openapi.yaml` 中記錄的公開 REST 路由。
- MCP how-to 中記錄的 MCP stdio 和 Streamable HTTP 工具行為。
- 面向 1.x 全新資料庫的 SQLite migrations；在這次 v1 基線之後，未來 1.x migration 保持追加式。
- 每筆記庫 git repository 布局和內容定址 blob 儲存。
- CLI 子命令和既有參數。
- Obsidian 外掛設定和同步行為，允許 1.x 正常新增向後相容功能。

OpenAPI 中沒有記錄的路由，例如 Admin Web UI 表單處理器，屬於內部實作細節。

## 推薦的 0.x 到 1.0 流程

1. 如條件允許，先把舊部署升級到最終 0.8.x patch 版本，然後僅用它完成備份、materialize 或匯出準備。
2. 執行 `pkvsyncd backup --output <backup-dir>` 並妥善保存備份。
3. 對每個筆記庫，使用最新 Obsidian 用戶端、`git clone`，或 `pkvsyncd materialize <vault-id> --output <dir>` 得到目前檔案樹。
4. 停止舊服務端。
5. 使用全新的空 `data_dir` 和 `metadata.db` 啟動 PKV Sync 1.0。
6. 完成 `/setup`，重新建立使用者和筆記庫，然後 push 或匯入 materialized 筆記庫內容。
7. 通知使用者把 Obsidian 外掛更新到 1.0.0。

## 外掛相容性

1.0 服務端的受支援外掛是隨服務端捆綁的 1.0 Obsidian 外掛。舊的 v0.8.x 外掛使用同一套核心同步 API，但新的修復和自更新加固只在 1.0+ 中維護。

## 相對 0.x 的破壞性變更

- 由於 migrations 已壓縮為單個 v1 基線，0.x SQLite 資料庫不能原地升級。
- 首次執行 setup 仍然透過瀏覽器完成；全新服務端不會再把隨機管理員密碼列印到日誌。

筆記內容、git 歷史和 blobs 仍可透過 backup/materialize/recreate/import 工作流帶到新部署。

## 已知注意事項

- 原生 per-vault E2EE 不屬於 1.0 範圍。今天需要客戶端側檔案內容加密的使用者可以使用 [`git-crypt`](./git-crypt-howto.zh-Hant.md)，並接受路徑仍為明文的取捨。
- `/metrics` 預設關閉；啟用後仍需生產認證門禁。
- 生產部署請設定 `public_host`。當服務端無法確定設定好的 HTTPS 公網 origin 時，admin POST 會故意 fail closed。
