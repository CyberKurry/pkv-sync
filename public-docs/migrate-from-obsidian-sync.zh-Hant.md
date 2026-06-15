# 從 Obsidian Sync 遷移

[English](./migrate-from-obsidian-sync.md) | [简体中文](./migrate-from-obsidian-sync.zh-CN.md) | 繁體中文 | [日本語](./migrate-from-obsidian-sync.ja.md) | [한국어](./migrate-from-obsidian-sync.ko.md)

文件版本：v1.4.3。

本文說明如何把已經使用 Obsidian Sync 的 Obsidian 筆記庫目前檔案匯入到新的 PKV Sync 筆記庫。

遷移只匯入目前裝置上現有的檔案。它不會匯入 Obsidian Sync 歷史、遠端版本歷史、已刪除檔案歷史或衝突中繼資料。PKV Sync 的歷史會從建立新 PKV 筆記庫的遷移提交開始。

遷移也不會停用、解除安裝或修改 Obsidian Sync。確認 PKV Sync 結果之後，如果你想停止使用 Obsidian Sync，請在 Obsidian 中手動關閉。

## 開始之前

- 先等待 Obsidian Sync 在用於遷移的裝置上完成同步。
- 遷移前手動備份整個筆記庫資料夾。
- 如有可能，匯入期間保持 Obsidian 關閉，或至少不要編輯檔案。
- 先建立或確認目標 PKV Sync 服務端帳號。

## 會匯入什麼

PKV Sync 會建立一個新筆記庫，並把目前匯入內容作為第一條 PKV 歷史提交。

普通 Markdown 檔案、附件和常規筆記庫檔案會被匯入，除非它們命中 PKV Sync 的強制排除規則。

## 會跳過什麼

匯入器會跳過 Obsidian Sync 內部檔案、PKV Sync 外掛自身狀態、OS 垃圾檔案以及本機執行階段檔案，包括：

- `.obsidian/sync/`
- `.obsidian/workspace.json`
- `.obsidian/workspace-mobile.json`
- `.obsidian/workspaces.json`
- `.obsidian/cache/**`
- `.obsidian/plugins/pkv-sync/`（外掛自身的設定與 token store 僅保留在本機）
- `.trash/**`
- `.git/**`
- `.DS_Store`（macOS）
- `Thumbs.db`（Windows）
- `*.tmp`、`*.lock` 等暫存檔案
- 裝置專屬的工作區、快取、回收站和暫存檔案

部分 `.obsidian` 設定檔之後可以透過按筆記庫 `.obsidian` allowlist 同步。相關規則請閱讀 `.obsidian` 設定同步指南。

## 遷移之後

在另一台裝置上開啟新的 PKV 筆記庫，確認筆記和附件看起來正確。檢查完成前，請保留手動備份。

如果你繼續讓 Obsidian Sync 和 PKV Sync 使用同一個資料夾，請謹慎修改檔案。兩個同步系統可能會同時操作同一批檔案，而 PKV Sync 只會記錄遷移提交之後收到的變更。
