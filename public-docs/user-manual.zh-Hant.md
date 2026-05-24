# PKV Sync 使用者手冊

[English](./user-manual.md) | [简体中文](./user-manual.zh-CN.md) | 繁體中文 | [日本語](./user-manual.ja.md) | [한국어](./user-manual.ko.md)

本文面向連接既有 PKV Sync 服務端的 Obsidian 使用者。開始前，請向服務端管理員取得服務端分享 URL，以及帳號或邀請碼。

## 手動安裝外掛

1. 從對應的 GitHub Release 下載 `pkv-sync-plugin.zip`。
2. 解壓到你的 Obsidian 倉庫：

   ```text
   <vault>/.obsidian/plugins/pkv-sync/
   ```

3. 在 Obsidian 中啟用社群外掛。
4. 啟用 **PKV Sync**。

解壓後的目錄應包含 `main.js`、`manifest.json` 和 `styles.css`。

## 外掛更新

PKV Sync 設定頁包含 **Updates** 區段。預設情況下，外掛會向已連接的 PKV Sync 服務端檢查內建外掛版本；這對自託管部署是首選來源，因為升級服務端也會發布匹配的外掛資源。服務端配置了 `public_host` 時，外掛資源 URL 會固定到該外部主機。需要時可以把更新來源切到 GitHub release。

有新版本時，點擊 **Update now** 會下載 `main.js`、`manifest.json` 和存在時的 `styles.css`，校驗 SHA-256 後寫入外掛檔案，並提示重新載入 Obsidian。命令面板也提供 **PKV Sync: Check for PKV Sync plugin updates**。

## 連接服務端

服務端分享 URL 通常類似：

```text
https://sync.example.com/k_xxx/
```

開啟 **Settings -> PKV Sync**，貼上分享 URL，然後點擊 **Connect**。如果 URL 中已經包含部署金鑰，外掛會自動填寫。

如果填錯服務端，或需要切換到另一個自託管服務端，可以在登入畫面點擊 **Change server** 回到服務端設定，無需重新安裝外掛。

## 登入或註冊

註冊流程取決於服務端執行階段設定：

- **Disabled**：管理員必須先建立帳號。
- **Invite only**：輸入管理員提供的邀請碼。
- **Open**：可以直接建立帳號。

登入後，選擇既有遠端筆記庫或建立新的遠端筆記庫。如果你把一個本機已經存在且內容與遠端完全一致的倉庫連接到該遠端倉庫，PKV Sync 會把匹配檔案納入本機同步索引，而不是產生一整庫衝突檔案。

## 同步行為

PKV Sync 在 Obsidian 內同步目前倉庫：

- 本機檔案變更會在短暫去抖後推送。
- 外掛會定期輪詢遠端變更。
- 設定頁和命令面板都可以手動同步。
- 相關檔案建立、修改、刪除事件會安排同步。
- 視窗失焦時可觸發同步。
- 啟動時會根據倉庫內容和本機同步索引識別未同步的本機變更。

上傳大型附件時，請保持 Obsidian 開啟。外掛連接後會讀取服務端設定，並使用服務端提供的文字副檔名清單和最大檔案大小規則。

## 選擇性 `.obsidian` 同步

PKV Sync 可以透過按筆記庫設定的 allowlist 同步部分 Obsidian 設定檔。新遠端筆記庫預設包含主題、CSS snippets、快捷鍵、應用程式偏好、外觀偏好和已啟用外掛清單的規則。

已有遠端筆記庫會保持空 allowlist，直到你主動 opt-in。在 **Settings -> PKV Sync** 中選擇目前筆記庫，編輯 **.obsidian sync rules**，然後保存。推薦起步清單按鈕會填入與新筆記庫相同的起步規則。

外掛程式碼和外掛設定預設不會同步。新增 `.obsidian/plugins/**` 或外掛 `data.json` 等進階規則前，請先閱讀 [`dot-obsidian-sync-howto.zh-Hant.md`](./dot-obsidian-sync-howto.zh-Hant.md)。

## 上次同步時間

設定頁會用相對時間顯示上次成功同步時間。點擊旁邊的小展開控制項可顯示精確時間，格式如下：

```text
YYYY/MM/DD HH:MM:SS
```

外掛使用你選擇的 IANA 時區，預設為 `Asia/Shanghai`。

## 歷史、差異和恢復

當服務端聲明支援歷史功能，並且外掛設定中的 **Enable history and diff UI** 處於開啟狀態時，可以從這些入口查看檔案歷史：

- **PKV Sync: Show file history**
- 檔案右鍵選單：**PKV Sync: File history**
- 檔案右鍵選單：**PKV Sync: Diff with previous**

歷史彈窗會按提交列出目前檔案的時間、裝置、commit id 和變更類型。文字檔案可以查看 unified diff。二進位檔案可以出現在歷史中並支援恢復，但 PKV Sync 不渲染二進位 diff。

恢復某個版本時，外掛會從服務端讀取所選歷史內容，寫回本機 Obsidian 倉庫，然後讓現有同步引擎把這次寫入作為新的普通提交推送。如果目前本機檔案與上次同步 hash 不一致，確認對話框會提示未同步的本機修改將被覆蓋。

PKV Sync 不會在外掛中保存完整離線歷史快取。歷史和 diff 視圖需要能夠連接服務端。

## 衝突檔案

如果兩台裝置離線編輯了同一個檔案，PKV Sync 會保留兩個版本。遠端或本機的替代版本會保存為產生的衝突檔案：

```text
note.md
note.conflict-2026-04-25-143022-Desktop.md
```

產生的衝突檔案不會繼續參與同步。請在 Obsidian 中查看兩個檔案，手動合併需要保留的內容，然後刪除衝突檔案。

可以透過以下入口管理產生的衝突檔案：

- **Settings -> PKV Sync -> Conflict files**
- **PKV Sync: List conflict files**
- **PKV Sync: Delete conflict files**

刪除動作只會處理 PKV Sync 產生格式的衝突檔案。類似 `my.conflict-resolution-notes.md` 的普通檔案仍會正常同步。

## 裝置 Token

登入會簽發 bearer 裝置 token。認證使用會續期該 token，因此活躍裝置會保持登入，連續 90 天未使用的裝置才會過期。外掛會保存穩定裝置 ID，因此同一裝置重新登入時會取代該裝置舊的活躍 token，而不是不斷累積重複 token。

Obsidian 外掛會把活躍 token 和部署金鑰保存於 `<vault>/.obsidian/plugins/pkv-sync/data.json`。請把這個檔案視為敏感檔案：保護明文備份和雲端同步目標，不要分享它。如果懷疑檔案已經洩露，請登出或讓管理員撤銷該裝置 token，然後重新連接。

- 可以在外掛設定中登出目前裝置。
- 裝置遺失後，請讓管理員撤銷對應裝置 token。
- 修改密碼會保留目前裝置登入狀態，並撤銷其他裝置 token。

## MCP 存取

如果管理員啟用了 `pkvsyncd mcp` 命令，AI 工具可以使用 bearer 裝置 token 透過 MCP 存取你的筆記庫。MCP 提供筆記庫清單、檔案清單、讀取 HEAD 或指定 commit 下的檔案、簡單文字搜尋，以及帶樂觀並發控制的顯式寫入／刪除工具。stdio 和 Streamable HTTP 設定範例請見 [`mcp-howto.zh-Hant.md`](./mcp-howto.zh-Hant.md)。

## 命令

PKV Sync 會新增以下命令面板動作：

- Show sync status
- Refresh account info
- Manual sync now
- View sync status details
- List conflict files
- Delete conflict files

## 隱私提醒

PKV Sync 不提供端到端加密。服務端管理員，以及任何擁有服務端檔案系統存取權限的人，都可以讀取已同步的倉庫內容和附件。請只連接你信任的服務端和管理員。
