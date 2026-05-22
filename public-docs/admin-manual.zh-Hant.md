# PKV Sync 管理員手冊

[English](./admin-manual.md) | [简体中文](./admin-manual.zh-CN.md) | 繁體中文 | [日本語](./admin-manual.ja.md) | [한국어](./admin-manual.ko.md)

本文涵蓋自託管 PKV Sync 服務端的日常管理。網路與主機加固請同時閱讀部署加固指南。

## 首次執行

1. 產生部署金鑰：

   ```bash
   pkvsyncd genkey
   ```

2. 以 `config.example.toml` 建立 `/etc/pkv-sync/config.toml`。
3. 執行資料庫 migration：

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml migrate up
   ```

4. 啟動服務端：

   ```bash
   pkvsyncd -c /etc/pkv-sync/config.toml serve
   ```

5. 保存輸出到 stderr 或容器日誌中的首次管理員密碼。
6. 開啟 `/admin/login`，使用 `admin` 登入並修改密碼。

已發布後的 migration 應保持追加式管理。對於既有部署，不要壓縮或編輯已經發布的 migration 檔案。

## Admin Web 面板

開啟：

```text
https://sync.example.com/admin/login
```

管理後台包含：

- 儀表板：系統、儲存、筆記庫、使用者和最近活動指標
- 使用者清單，支援搜尋和狀態篩選
- 使用者詳情頁：重設密碼、啟用/停用、管理員權限控制和 token 查看
- 全域裝置 token 頁面，可列出、建立和撤銷 token
- 筆記庫卡片：所有者、檔案數、大小、上次同步、元資料修復、刪除操作和按筆記庫同步設定
- 唯讀筆記庫檔案瀏覽器，支援檔案預覽、單檔案歷史時間線和 unified diff 渲染
- 邀請碼建立，可選過期時間、活躍邀請碼清單，以及刪除未使用邀請碼
- 執行階段設定，分為 General、Security、Sync & Storage、Network
- 活動日誌，支援按使用者和動作真實篩選 push/pull 以及筆記庫生命週期記錄
- Blob 垃圾回收觸發
- 英文和簡體中文語言切換

時間戳、持續時間、位元組大小、執行時間和活動資料都會以人類可讀形式顯示。預設時區是 `Asia/Shanghai`，可在設定中修改。

## 使用者管理

- 可在 **Users** 頁面或 CLI 建立使用者。
- 使用者名稱必須是 3-32 個 ASCII 字母、數字、`_`、`-` 或 `.`。
- 使用者頁面的搜尋和狀態篩選可以縮小表格範圍。
- 開啟使用者詳情頁可重設密碼、啟用或停用帳號、提升或降低管理員權限，並查看該使用者的裝置 token。
- 如果後續可能需要稽核歷史，優先停用使用者而不是刪除使用者。
- 不要降級或停用最後一個活躍管理員帳號。

從 Admin WebUI 重設密碼會撤銷該使用者已有裝置 token。使用者需要重新登入。

CLI 備援命令：

```bash
pkvsyncd -c /etc/pkv-sync/config.toml user add alice
pkvsyncd -c /etc/pkv-sync/config.toml user add alice --admin
pkvsyncd -c /etc/pkv-sync/config.toml user passwd alice
pkvsyncd -c /etc/pkv-sync/config.toml user list
pkvsyncd -c /etc/pkv-sync/config.toml user set-active alice --active false
```

## 裝置 Token

裝置 bearer token 會在認證請求時續期，連續 90 天未使用才會過期。使用者可以撤銷自己的 token，管理員可以撤銷任意使用者的 token。

維運注意事項：

- Token 明文只在建立時顯示一次。
- 資料庫只保存 SHA-256 token hash。
- 每次認證請求都會把 token 過期時間延長到該請求時間之後 90 天，並且不會縮短更晚的過期時間。
- 同一穩定外掛裝置 ID 再次登入時，會取代該裝置舊的活躍 token。
- 被活動記錄引用的已撤銷 token 可以清理，同時保留活動歷史。

## 筆記庫

刪除筆記庫會移除：

- 筆記庫資料庫列
- 從該列級聯的相關元資料
- `data_dir/vaults/<vault-id>` 下的後端裸 Git 倉庫
- 記憶體中的按筆記庫 push 鎖

Blob 檔案是內容定址的，可能會保留到垃圾回收確認其超過寬限期且不再被引用。

如果中斷操作後檔案數、大小或 blob 引用看起來不正確，可以使用筆記庫元資料修復。

### 按筆記庫同步設定

在 **Vaults** 頁面點擊某個筆記庫卡片上的 **Settings**，可以編輯該筆記庫的 `extra_sync_globs` allowlist。它控制哪些隱藏路徑，包括選定的 `.obsidian` 設定檔，可以參與同步。

新筆記庫會自動獲得推薦起步 allowlist。已有筆記庫保持空設定，直到管理員或筆記庫所有者套用起步清單。**Apply starter allowlist** 會寫入推薦清單，包括主題、CSS snippets、快捷鍵、應用程式偏好、外觀偏好和已啟用外掛清單。

### 唯讀檔案歷史

在 **Vaults** 頁面點擊某個筆記庫卡片上的 **Browse files**。檔案瀏覽器會列出目前 HEAD 中的檔案、大小以及文字/二進位類型。開啟檔案後，文字檔案會顯示唯讀預覽，並提供 **History** 和 **Diff with previous** 連結。

歷史頁會列出該檔案相關的提交，並提供「查看該提交時的檔案」和對應 diff 的連結。diff 頁會按行渲染 unified diff，並用顏色區分新增、刪除和 hunk。二進位檔案只顯示元資料，不渲染二進位 diff 內容。

瀏覽檔案、歷史和 diff 會記錄 `view_commit`、`view_history` 和 `view_diff` 活動。Admin history 中提供筆記庫 rollback 控制；請在確認目標提交後再使用，因為 rollback 會從選定歷史點建立新的筆記庫狀態。

## 邀請碼和註冊

可從 **Settings** 設定註冊模式：

- `disabled`：只允許管理員建立帳號
- `invite_only`：使用者使用邀請碼註冊
- `open`：任何擁有部署 URL 的人都可以註冊

建立邀請碼時可以填寫未來過期時間。Admin WebUI 使用人類可讀日期時間輸入，內部仍儲存 Unix 秒。已使用邀請碼不能透過 admin API 刪除，應保留用於稽核歷史。

只有在短時間視窗或具備額外監控和限流的公開部署中，才建議使用 `open`。

## 執行階段設定

設定頁編輯保存在 SQLite 中的設定值。改動對新請求立即生效；保存時會刷新記憶體快取。

**通用** — 服務名稱、預設時區、`enable_metrics` 指標開關。開啟後 `/metrics` 可用，但仍需要部署金鑰中介軟體、外掛 User-Agent guard 和管理員 bearer token。

**安全** — 註冊模式（`disabled` / `invite_only` / `open`）、登入失敗閾值、失敗視窗和鎖定時長。登入速率限制器同時計算已失敗次數和進行中的密碼驗證，並發暴力嘗試無法繞過閾值。認證同步 API 路由另有固定視窗限流：按路由、方法、用戶端 IP 和 bearer 裝置 token 分桶，每 60 秒最多 600 次請求。

**同步與儲存**
- 最大檔案大小（預設 `100 MiB`）
- 支援的文字副檔名 — 清單外的檔案按二進位 blob 處理
- 額外 exclude glob — 管理員可調，補充內建的 `.obsidian/`、`.trash/`、`.conflict-*`、`.git/` 排除清單
- 歷史介面和 diff 端點開關
- **Push 去抖**（`push_debounce_ms`，預設 `250`）：本機編輯穩定到推送之間的延遲。變小可縮短端到端延遲，變大可每次 push 合併更多按鍵
- **SSE 內聯內容上限**（`inline_content_max_bytes`，預設 `8192`，上限 `65536`）：此尺寸以內的文字變更隨 SSE 事件直接下發，接收端外掛無需再 pull；超過則降級走 pull
- **SSE 心跳**（`sse_heartbeat_seconds`，預設 `30`）：事件流保活，避免閒置 SSE 連線被反向代理切斷。並發 SSE 訂閱預設按使用者限制為 16，並保留 1024 的全域上限。
- **Git smart HTTP**（`enable_git_smart_http`，預設關）：開啟後授權裝置可 `git clone https://_:<token>@host/git/<vault-id>`。伺服器還需要 `PATH` 中有 `git` 二進位；公開的 `/api/config` 能力兩個條件都滿足才顯示為可用

## 活動日誌

活動日誌記錄 push、pull、create_vault、delete_vault、view_commit、view_history、view_diff 等同步、筆記庫生命週期與唯讀瀏覽操作，包括：

- 使用者
- 筆記庫
- 動作
- 裝置名稱
- 檔案數
- 位元組大小
- 用戶端 IP
- User-Agent
- 詳情
- 時間戳

使用活動篩選可以檢查特定使用者或操作類型。

`create_vault` 和 `delete_vault` 來自管理面板、外掛和 API 的筆記庫建立／刪除操作。

## 分享服務端 URL

分享服務端或 Admin WebUI 提供的 URL：

```text
https://sync.example.com/k_xxx/
```

請把它視為敏感資訊。它不是使用者密碼，但包含部署金鑰，是外掛 API 流量的第一道預認證入口。

## 維護清單

- 使用 `pkvsyncd backup --output <dir> [--data-dir <dir>] [--gzip]` 產生維運快照。輸出目錄必須不存在或為空；命令會用 `VACUUM INTO` 快照 SQLite，複製 `vaults/`、`blobs/` 和存在時的 `config.toml`，並寫入帶 pkvsyncd 版本、元件雜湊、大小和數量的 `MANIFEST.json`。
- 使用 `pkvsyncd restore --input <backup-dir> --data-dir <dir>` 恢復到不存在或為空的資料目錄。只有確認目標可以先清空時才加 `--force`；恢復會先校驗 manifest 雜湊，複製完成後自動執行 verify。
- 維護後或主機儲存異常後執行 `pkvsyncd verify [--data-dir <dir>]`。它會檢查被引用的 blob 檔案，報告孤立 blob，用 `git2` 校驗筆記庫 git 倉庫，並在缺失、損壞或 git 錯誤時返回失敗。`--no-fail` 會保留報告但強制返回成功退出碼。
- 大量刪除附件後執行 blob 垃圾回收。
- 關注日誌和活動中重複出現的 `401`、`403`、`404`、`409` 和 `429` 回應。
- 保持服務端二進位、外掛包、Docker 映像、反向代理和主機系統及時更新。
- 打 tag 發版前確認 CI 通過。
- 檢查每個 release 都包含 Linux amd64、Linux arm64、Windows x64、外掛 zip、校驗和和 GHCR Docker 映像 tag。
