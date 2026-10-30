# PKV Sync

PKV Sync 是一套自架、Git 原生的 Obsidian 筆記庫同步伺服器。

[![CI](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml/badge.svg)](https://github.com/cyberkurry/pkv-sync/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](./LICENSE)

文件版本：v1.7.1。

[English](./README.md) · [简体中文](./README.zh-CN.md) · 繁體中文 · [日本語](./README.ja.md) · [한국어](./README.ko.md)

## 狀態

PKV Sync 1.7.1 是目前的穩定修補版本，改善多裝置同步收斂、衝突內容保留與 SSE 復原，修復官方 Docker 映像中的 Git 複製／擷取及活動頁文字篩選，並更新外掛建置相依套件。

## 功能

### 同步

- **多使用者、多筆記庫**同步，以裝置 Token 驗證身分，每個筆記庫各有推送鎖，重試冪等。
- **即時推送。** 小幅改動透過 Server-Sent Events 在亞秒級送達；輪詢作為兜底手段保留。
- **以 Git 為唯一事實來源。** 每個筆記庫都是一個裸 git 儲存庫，因此單檔歷程、統一 diff、單檔還原開箱即用——外掛裡和管理後台裡都能用。
- **衝突不會毀資料。** 外掛絕不靜默覆寫本機改動；衝突會以 `.conflict-*` 檔案的形式浮現，搭配一鍵解決器。

### 維運

- **管理後台**支援五種語言（English、简中、繁中、日本語、한국어），涵蓋使用者、裝置 Token、筆記庫、邀請碼、活動日誌和 blob 垃圾回收，對破壞性的筆記庫操作與使用者操作都會跳出確認對話框。
- **筆記庫可被 AI 讀取。** MCP 透過 stdio 或 `pkvsyncd serve` 內嵌的 `/mcp` 路由（預設開啟）對外提供讀寫工具，而且每個版本都隨倉庫附帶可直接使用的 [MCP Registry 條目](./mcp/server.json)（`io.github.cyberkurry/pkv-sync`）。
- **為 LLM 維護的 wiki 而生。** 除了讀寫，agent 還能按 frontmatter 檢索、精確分頁、按層 lint 連結、找出「來源已過期」的頁面，並在「dry run + 內容雜湊守衛」之後才寫入——參見 [LLM Wiki 工作流程](./public-docs/llm-wiki-howto.zh-Hant.md)。
- **預設就有邊界約束。** 管理員建立的密碼走 setup 級強密碼原則，Token 密文只顯示一次，上傳和 MCP 回應都有大小上限，常駐的 SSE 串流會重新查驗已撤銷的 Token。

### 設計

- **刻意選用成熟技術。** 單一執行檔、單一 SQLite 中繼資料庫、每個筆記庫一個裸 git 儲存庫、每個附件一個內容定址 blob。

## 用 Docker Compose 快速開始

推薦走這條路。`deploy/caddy/` 裡的 Caddy 透過 Let's Encrypt 在前面擋 HTTPS；PKV Sync 在 compose 網路內部監聽 `127.0.0.1:6710`，永遠不會從公網直接收到明文 HTTP。

你需要一個網域名稱（例如 `sync.example.com`），其 A/AAAA 記錄指向這台伺服器，並且公網能存取 `80` 和 `443` 連接埠（80 連接埠用於 ACME HTTP-01 校驗）。

### 部署整套服務

1. 產生部署金鑰：

   ```bash
   docker run --rm ghcr.io/cyberkurry/pkv-sync:latest genkey
   ```

2. 在 `docker-compose.yml` 旁邊放一份 `config.toml`：

   ```toml
   [server]
   bind_addr      = "0.0.0.0:6710"
   deployment_key = "k_0123456789abcdef0123456789abcdef"  # 換成 genkey 的輸出
   public_host    = "sync.example.com"   # 必填，管理後台的 POST 才能正常運作

   [storage]
   data_dir = "/var/lib/pkv-sync"
   db_path  = "/var/lib/pkv-sync/metadata.db"

   [network]
   trusted_proxies = ["172.16.0.0/12"]   # Docker 橋接網段

   [mcp]
   embed_in_serve = true                 # 預設值；設為 false 可關閉
   ```

3. 編輯 `deploy/caddy/Caddyfile`，把 `sync.example.com` 換成你的真實網域名稱。

4. 啟動整套服務：

   ```bash
   docker compose up -d
   ```

   在瀏覽器裡打開 `https://sync.example.com/setup`，建立第一個管理員帳號。

5. 在 Obsidian 裡安裝 `pkv-sync-plugin.zip`
   （裝到 `<vault>/.obsidian/plugins/pkv-sync/`），啟用它，從管理後台複製分享
   URL 貼進去，然後登入或註冊並選擇一個筆記庫。

### 更新

```bash
docker compose pull && docker compose up -d
```

### 接下來讀什麼

原生安裝、反向代理調校（Caddy / Nginx / Traefik）、`public_host` 的含義、備份與還原、磁碟加密，這些內容見[部署加固指南](./public-docs/deployment-hardening.zh-Hant.md)。

## MCP 部署模式

MCP HTTP 只提供內嵌方式：`[mcp].embed_in_serve` 預設為 `true`，因此 `pkvsyncd serve` 會把 `/mcp` 掛載到主伺服器連接埠上，共用同一套 TLS 終結、反向代理、部署金鑰和 bearer Token 驗證。設為 `embed_in_serve = false` 可以關閉它。獨立的 `pkvsyncd mcp --transport http --bind` 行程已在本版本中移除——現有的獨立部署請改為指向主伺服器的 `/mcp`。本機 AI 工具也可以走 stdio 傳輸：`pkvsyncd mcp --vault <id>`。

## Obsidian 外掛

本機檔案是唯一事實來源——外掛直接讀寫你磁碟上正常的 Obsidian 筆記庫，不設代理檔案系統。非敏感的外掛設定和同步索引放在 `<vault>/.obsidian/plugins/pkv-sync/data.json`；登入狀態、目前生效的 bearer 裝置 Token、部署金鑰和穩定的裝置識別則放在 Obsidian 的裝置本機儲存裡。請把 Obsidian 的裝置本機儲存、明文備份，以及舊版本遺留的外掛 `data.json` 副本一併視為機敏資料。裝置 Token 在使用時續期，閒置 90 天後過期，絕對有效期 365 天；在同一台裝置上重新登入會輪換目前生效的 Token。

日常功能——命令面板、檔案歷程、並排 diff、衝突解決、`.obsidian` 選擇性同步、裝置管理和自動更新——在[使用者手冊](./public-docs/user-manual.zh-Hant.md)裡逐一講解。

## 目前的加密狀況

PKV Sync 1.0 **尚未**提供原生的端到端加密——伺服器能夠讀取筆記庫內容。按筆記庫開啟的原生 E2EE 已列入 1.x 路線圖，將作為可選模式提供，因為加密會讓伺服器端那些讓 Git 原生 PKV 真正有用的能力全部失效（歷程 diff、三方自動合併、SSE 內嵌酬載、MCP 讀寫）。

如果在這項能力落地之前你就需要 E2EE，可以在筆記庫上疊一層 [`git-crypt`](https://github.com/AGWA/git-crypt)：被標記的路徑到達伺服器時是不透明密文 blob，伺服器無法解密。檔案名稱在伺服器上仍是明文（對多數威脅模型而言可以接受）。持有金鑰的客戶端依然可以用標準 `git clone` 和 `pkvsyncd materialize`。

真實部署還應額外做到：跑在 HTTPS 後面、收緊 `trusted_proxies`、加密資料磁碟、加密備份——參見[部署加固指南](./public-docs/deployment-hardening.zh-Hant.md)。

## 文件

| 主題 | 文件 |
| --- | --- |
| 外掛的日常使用 | [使用者手冊](./public-docs/user-manual.zh-Hant.md) |
| 伺服器管理與執行階段設定 | [管理員手冊](./public-docs/admin-manual.zh-Hant.md) |
| 每一個 CLI 命令與參數 | [CLI 參考](./public-docs/cli-reference.zh-Hant.md) |
| 從 0.x 升級到 1.0 | [1.0 升級說明](./public-docs/upgrade-notes-v1.0.zh-Hant.md) |
| 反向代理、TLS、備份、加固 | [部署加固](./public-docs/deployment-hardening.zh-Hant.md) |
| HTTP API 契約 | [OpenAPI 規範](./public-docs/openapi.yaml) |
| MCP 設定與工具清單 | [MCP 操作指南](./public-docs/mcp-howto.zh-Hant.md) |
| 由 LLM 維護的 wiki 工作流 | [LLM Wiki 操作指南](./public-docs/llm-wiki-howto.zh-Hant.md) |
| 從 Obsidian Sync 遷移 | [遷移指南](./public-docs/migrate-from-obsidian-sync.zh-Hant.md) |
| 安全性漏洞回報 | [SECURITY.md](./SECURITY.md) |
| 發布歷史 | [CHANGELOG.md](./CHANGELOG.md) |

## 開發

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace

cd plugin
npm run typecheck
npm exec vitest run
npm run build
```

CI 在 Linux 和 Windows 上跑完整的 Rust 矩陣，外加外掛的 test／typecheck／build／package、一次 Docker 建置，以及發布執行檔的冒煙測試。

## 參與貢獻

歡迎提 bug、提需求和發 Pull Request。除了修 bug 之外的事情，請先開 issue 討論。建置與測試命令、提交與發布流程，以及 CI 會校驗的文件規則，見 [CONTRIBUTING.md](./CONTRIBUTING.md)。

## 授權條款

AGPL-3.0-only。見 [LICENSE](./LICENSE)。
